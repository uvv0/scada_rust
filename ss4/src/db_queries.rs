//! Модуль SQL-запросов для загрузки конфигурации, записи телеметрии и работы с alarm/arx состоянием.

use anyhow::Result;
use tokio_postgres::{Client, Row};
use tracing::debug;

use crate::reg::Reg;
use crate::types::{
    AlarmNotifyRoute, AlarmRule, ArxValRow, ConnInfo, GScriptRow, KpzRow, ObjRow, ScriptBindingRow,
};

#[derive(Clone, Copy, Debug)]
/// Runtime-параметры планировщика, загружаемые из `scheduler_runtime_cfg`.
pub struct SchedulerRuntimeCfg {
    pub no_response_failures: u8,
    pub no_response_backoff_sec: u64,
    pub metrics_p95_warn_ms: u64,
    pub metrics_p95_crit_ms: u64,
    pub modbus_a_timeout_ms: u64,
    pub modbus_script_timeout_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Лёгкий token topology-таблиц для cheap precheck перед full reload.
/// Предпочитает `max(updated_at)`, а при отсутствии колонок падает обратно на aggregate fingerprint.
pub struct TopologyFingerprint {
    pub kpz_sig: i64,
    pub obj_sig: i64,
    pub ip_sig: i64,
    pub port_sig: i64,
    pub n_mb_sig: i64,
    pub reg_sig: i64,
    pub g_script_sig: i64,
    pub binding_sig: i64,
}

async fn load_updated_at_sig(client: &Client, table: &str, id_col: &str) -> Result<Option<i64>> {
    let sql = format!(
        "select
            count(*)::int8,
            coalesce(max({id_col}), 0)::int8,
            coalesce(extract(epoch from max(updated_at))::bigint, 0)::int8
         from {table}"
    );
    match client.query_one(&sql, &[]).await {
        Ok(r) => {
            let count: i64 = r.get(0);
            let max_id: i64 = r.get(1);
            let max_updated_at: i64 = r.get(2);
            Ok(Some(count ^ (max_id << 1) ^ (max_updated_at << 2)))
        }
        Err(e) => {
            let missing_updated_at = e
                .as_db_error()
                .map(|d| d.code().code() == "42703")
                .unwrap_or(false);
            let missing_table = e
                .as_db_error()
                .map(|d| d.code().code() == "42P01")
                .unwrap_or(false);
            if missing_updated_at || missing_table {
                return Ok(None);
            }
            Err(e.into())
        }
    }
}

async fn load_lookup_sig(client: &Client, table: &str) -> Result<i64> {
    if let Some(sig) = load_updated_at_sig(client, table, "id").await? {
        return Ok(sig);
    }
    let sql = format!(
        "select
            count(*)::int8,
            coalesce(max(id), 0)::int8,
            coalesce(sum(id::int8 + length(coalesce(name, ''))::int8), 0)::int8
         from {table}"
    );
    let row = client.query_one(&sql, &[]).await?;
    let count: i64 = row.get(0);
    let max_id: i64 = row.get(1);
    let sum_sig: i64 = row.get(2);
    Ok(count ^ (max_id << 1) ^ (sum_sig << 2))
}

/// Загружает cheap token topology-таблиц, чтобы пропускать full reload при отсутствии изменений.
pub async fn load_topology_fingerprint(client: &Client) -> Result<TopologyFingerprint> {
    let kpz_sig = if let Some(sig) = load_updated_at_sig(client, "kpz", "id").await? {
        sig
    } else {
        match client
            .query_one(
                "select
                count(*)::int8,
                coalesce(max(id), 0)::int8,
                coalesce(sum(
                    id::int8 + rtu::int8 + obj::int8 + modem::int8 + max_pkt_len::int8 +
                    start::int8 + t_a::int8 + t_script::int8 +
                    case when coalesce(en_post, false) then 1 else 0 end
                ), 0)::int8
             from kpz",
                &[],
            )
            .await
        {
            Ok(r) => {
                let count: i64 = r.get(0);
                let max_id: i64 = r.get(1);
                let sum_sig: i64 = r.get(2);
                count ^ (max_id << 1) ^ (sum_sig << 2)
            }
            Err(_) => {
                let r = client
                    .query_one(
                        "select
                        count(*)::int8,
                        coalesce(max(id), 0)::int8,
                        coalesce(sum(
                            id::int8 + rtu::int8 + obj::int8 + modem::int8 + max_pkt_len::int8 +
                            start::int8 + t_a::int8 + t_script::int8
                        ), 0)::int8
                     from kpz",
                        &[],
                    )
                    .await?;
                let count: i64 = r.get(0);
                let max_id: i64 = r.get(1);
                let sum_sig: i64 = r.get(2);
                count ^ (max_id << 1) ^ (sum_sig << 2)
            }
        }
    };

    let obj_sig = if let Some(sig) = load_updated_at_sig(client, "obj", "id").await? {
        sig
    } else {
        let obj_row = client
            .query_one(
                "select
                    count(*)::int8,
                    coalesce(max(id), 0)::int8,
                    coalesce(sum(
                        id::int8 +
                        coalesce(kanal, 0)::int8 + coalesce(speed, 0)::int8 +
                        coalesce(stop, 0)::int8 + coalesce(parit, 0)::int8 +
                        coalesce(bit, 0)::int8 +
                        length(coalesce(ip, ''))::int8 +
                        length(coalesce(port::text, ''))::int8
                    ), 0)::int8
                 from obj",
                &[],
            )
            .await?;
        let count: i64 = obj_row.get(0);
        let max_id: i64 = obj_row.get(1);
        let sum_sig: i64 = obj_row.get(2);
        count ^ (max_id << 1) ^ (sum_sig << 2)
    };

    let ip_sig = load_lookup_sig(client, "ip").await?;
    let port_sig = load_lookup_sig(client, "port").await?;
    let n_mb_sig = match load_lookup_sig(client, "n_mb").await {
        Ok(sig) => sig,
        Err(e) => {
            let undef_table = e
                .downcast_ref::<tokio_postgres::Error>()
                .and_then(|pg| pg.as_db_error())
                .map(|d| d.code().code() == "42P01")
                .unwrap_or(false);
            if undef_table {
                0
            } else {
                return Err(e);
            }
        }
    };

    let reg_sig = if let Some(sig) = load_updated_at_sig(client, "reg", "id").await? {
        sig
    } else {
        let reg_row = client
            .query_one(
                "select
                    count(*)::int8,
                    coalesce(max(id), 0)::int8,
                    coalesce(sum(
                        id::int8 + mb::int8 + tip::int8 +
                        coalesce(n_mb, 0)::int8 + coalesce(bits, 0)::int8 +
                        coalesce(grup, 0)::int8 +
                        coalesce(a_no_write, 0)::int8 +
                        case when a_en::text in ('1','t','true','T','TRUE') then 1 else 0 end
                    ), 0)::int8
                 from reg",
                &[],
            )
            .await?;
        let count: i64 = reg_row.get(0);
        let max_id: i64 = reg_row.get(1);
        let sum_sig: i64 = reg_row.get(2);
        count ^ (max_id << 1) ^ (sum_sig << 2)
    };

    let g_script_sig = if let Some(sig) = load_updated_at_sig(client, "g_script", "grup").await? {
        sig
    } else {
        let g_script_row = client
            .query_one(
                "select
                    count(*)::int8,
                    coalesce(max(grup), 0)::int8,
                    coalesce(sum(
                        grup::int8 +
                        coalesce(max_k, 0)::int8 + coalesce(max, 0)::int8 +
                        coalesce(ver, 0)::int8 +
                        case when coalesce(en, false) then 1 else 0 end +
                        length(coalesce(pre_src, ''))::int8 +
                        length(coalesce(post_src, ''))::int8
                    ), 0)::int8
                 from g_script",
                &[],
            )
            .await?;
        let count: i64 = g_script_row.get(0);
        let max_id: i64 = g_script_row.get(1);
        let sum_sig: i64 = g_script_row.get(2);
        count ^ (max_id << 1) ^ (sum_sig << 2)
    };

    let binding_sig =
        if let Some(sig) = load_updated_at_sig(client, "script_binding", "kpz_id").await? {
            sig
        } else {
            let binding_row = match client
                .query_one(
                    "select
                    count(*)::int8,
                    coalesce(max(kpz_id), 0)::int8,
                    coalesce(sum(
                        kpz_id::int8 + grup::int8 + logical::int8 +
                        coalesce(reg_id, 0)::int8 + coalesce(addr, 0)::int8 +
                        case when coalesce(enabled, true) then 1 else 0 end
                    ), 0)::int8
                 from script_binding",
                    &[],
                )
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let undef_table = e
                        .as_db_error()
                        .map(|d| d.code().code() == "42P01")
                        .unwrap_or(false);
                    if undef_table {
                        return Ok(TopologyFingerprint {
                            kpz_sig,
                            obj_sig,
                            ip_sig,
                            port_sig,
                            n_mb_sig,
                            reg_sig,
                            g_script_sig,
                            binding_sig: 0,
                        });
                    }
                    return Err(e.into());
                }
            };
            let count: i64 = binding_row.get(0);
            let max_id: i64 = binding_row.get(1);
            let sum_sig: i64 = binding_row.get(2);
            count ^ (max_id << 1) ^ (sum_sig << 2)
        };

    Ok(TopologyFingerprint {
        kpz_sig,
        obj_sig,
        ip_sig,
        port_sig,
        n_mb_sig,
        reg_sig,
        g_script_sig,
        binding_sig,
    })
}

/// Загружает список КПЗ для планировщика.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
///
/// # Returns
/// - `Ok(Vec<KpzRow>)`: строки `kpz`.
/// - `Err(...)`: ошибка SQL/декодирования.
pub async fn load_kpz_rows(client: &Client) -> Result<Vec<KpzRow>> {
    let (rows, has_en_post) = match client
        .query(
            "select id, name, rtu, obj, modem, grups, max_pkt_len, start, t_a, t_script, \
             case when coalesce(en_post, false) then 1 else 0 end as en_post_i \
             from kpz order by id",
            &[],
        )
        .await
    {
        Ok(rows) => (rows, true),
        Err(e) => {
            tracing::warn!(err = %e, "kpz.en_post is not available; fallback to en_post=true");
            let rows = client
                .query(
                    "select id, name, rtu, obj, modem, grups, max_pkt_len, start, t_a, t_script from kpz order by id",
                    &[],
                )
                .await?;
            (rows, false)
        }
    };
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let rtu = get_i32(&r, 2)?;
        let obj = get_i32(&r, 3)?;
        let modem = get_i32(&r, 4)?;
        let max_pkt_len = get_i32(&r, 6)?;
        let start = get_i32(&r, 7)?;
        let t_a = get_i32(&r, 8)?;
        let t_script = get_i32(&r, 9)?;
        let en_post = if has_en_post {
            get_i32(&r, 10)? != 0
        } else {
            true
        };
        out.push(KpzRow {
            id: get_i32(&r, 0)?,
            name: get_text_opt(&r, 1),
            rtu,
            obj,
            modem,
            grups: r.get::<_, Vec<u8>>(5),
            max_pkt_len,
            start,
            t_a,
            t_script,
            en_post,
        });
    }
    Ok(out)
}

/// Загружает список объектов связи (`obj`) с сетевыми параметрами.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
///
/// # Returns
/// - `Ok(Vec<ObjRow>)`: строки `obj`.
/// - `Err(...)`: ошибка SQL/декодирования.
pub async fn load_obj_rows(client: &Client) -> Result<Vec<ObjRow>> {
    let rows = client
        .query(
            "select id, name, ip, port, kanal, speed, stop, parit, bit from obj order by id",
            &[],
        )
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(ObjRow {
            id: get_i32(&r, 0)?,
            name: get_text_opt(&r, 1),
            ip: get_text_opt(&r, 2),
            port: get_text_opt(&r, 3),
            kanal: get_i32_opt(&r, 4),
            speed: get_i32_opt(&r, 5),
            stop: get_i32_opt(&r, 6),
            parit: get_i32_opt(&r, 7),
            bit: get_i32_opt(&r, 8),
        });
    }
    Ok(out)
}

/// Загружает справочник `(id, name)` из разрешённых таблиц (`ip`, `port`, `n_mb`).
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
/// - `table`: имя разрешённой таблицы-справочника.
///
/// # Returns
/// - `Ok(Vec<(i32, String)>)`: элементы справочника.
/// - `Err(...)`: недопустимая таблица или ошибка SQL.
pub async fn load_items(client: &Client, table: &str) -> Result<Vec<(i32, String)>> {
    let sql = match table {
        "ip" => "select id, name from ip order by id",
        "port" => "select id, name from port order by id",
        "n_mb" => "select id, name from n_mb order by id",
        _ => {
            return Err(anyhow::anyhow!(
                "load_items: table is not allowed: {}",
                table
            ))
        }
    };
    let rows = client.query(sql, &[]).await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get::<_, i32>(0), r.get::<_, String>(1)))
        .collect())
}

fn get_text_opt(row: &Row, idx: usize) -> Option<String> {
    if let Ok(v) = row.try_get::<_, Option<String>>(idx) {
        return v;
    }
    if let Ok(v) = row.try_get::<_, Option<i32>>(idx) {
        return v.map(|n| n.to_string());
    }
    if let Ok(v) = row.try_get::<_, Option<i64>>(idx) {
        return v.map(|n| n.to_string());
    }
    None
}

fn get_i32_opt(row: &Row, idx: usize) -> Option<i32> {
    if let Ok(v) = row.try_get::<_, Option<i32>>(idx) {
        return v;
    }
    if let Ok(v) = row.try_get::<_, Option<i64>>(idx) {
        return v.map(|n| n as i32);
    }
    if let Ok(v) = row.try_get::<_, Option<String>>(idx) {
        return v.and_then(|s| s.parse::<i32>().ok());
    }
    None
}

fn get_i32(row: &Row, idx: usize) -> Result<i32> {
    if let Ok(v) = row.try_get::<_, i32>(idx) {
        return Ok(v);
    }
    if let Ok(v) = row.try_get::<_, Option<i32>>(idx) {
        return v.ok_or_else(|| anyhow::anyhow!("null i32 at column {}", idx));
    }
    if let Ok(v) = row.try_get::<_, i64>(idx) {
        return Ok(v as i32);
    }
    if let Ok(v) = row.try_get::<_, Option<i64>>(idx) {
        return v
            .map(|n| n as i32)
            .ok_or_else(|| anyhow::anyhow!("null i64 at column {}", idx));
    }
    if let Ok(v) = row.try_get::<_, String>(idx) {
        return v
            .parse::<i32>()
            .map_err(|e| anyhow::anyhow!("invalid i32 string at column {}: {}", idx, e));
    }
    if let Ok(v) = row.try_get::<_, Option<String>>(idx) {
        let s = v.ok_or_else(|| anyhow::anyhow!("null string at column {}", idx))?;
        return s
            .parse::<i32>()
            .map_err(|e| anyhow::anyhow!("invalid optional i32 string at column {}: {}", idx, e));
    }
    Err(anyhow::anyhow!(
        "failed to decode integer column {}: unsupported type",
        idx
    ))
}

/// Загружает конфигурацию регистров из `reg`.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
///
/// # Returns
/// - `Ok(Vec<Reg>)`: список регистров.
/// - `Err(...)`: ошибка SQL/декодирования.
pub async fn load_regs(client: &Client) -> Result<Vec<Reg>> {
    let rows = client
        .query(
            "select id, name, mb, n_mb, tip, bits, grup, \
             case when a_en::text in ('1','t','true','T','TRUE') then 1 else 0 end as a_en_i, \
             coalesce(a_no_write, 0) \
             from reg order by id",
            &[],
        )
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let a_en_val: i32 = r.get::<_, i32>(7);
        out.push(Reg {
            id: r.get::<_, i32>(0),
            name: r.get::<_, String>(1),
            addr: r.get::<_, i32>(2),
            n_mb: r.get::<_, Option<i32>>(3),
            tip: r.get::<_, i32>(4),
            bits: r.get::<_, Option<i32>>(5),
            grup: r.get::<_, Option<i32>>(6),
            a_en: a_en_val != 0,
            a_no_write: r.get::<_, i32>(8),
        });
    }
    Ok(out)
}

/// Загружает шаблоны скриптов групп из `g_script`.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
///
/// # Returns
/// - `Ok(Vec<GScriptRow>)`: строки скриптов групп.
/// - `Err(...)`: ошибка SQL/декодирования.
pub async fn load_g_script_rows(client: &Client) -> Result<Vec<GScriptRow>> {
    let rows = client
        .query(
            "select grup, pre_src, post_src, max_k, max, en, ver from g_script order by grup",
            &[],
        )
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(GScriptRow {
            grup: r.get::<_, i32>(0),
            pre_src: r.get::<_, Option<String>>(1),
            post_src: r.get::<_, Option<String>>(2),
            max_k: r.get::<_, Option<i32>>(3),
            max_words: r.get::<_, Option<i32>>(4),
            en: r.get::<_, Option<bool>>(5),
            ver: r.get::<_, Option<i32>>(6),
        });
    }
    Ok(out)
}

/// Загружает включённые привязки `script_binding` (logical -> reg/addr).
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
///
/// # Returns
/// - `Ok(Vec<ScriptBindingRow>)`: список привязок.
/// - `Err(...)`: ошибка SQL/декодирования.
pub async fn load_script_bindings(client: &Client) -> Result<Vec<ScriptBindingRow>> {
    let rows = client
        .query(
            "select kpz_id, grup, logical, reg_id, addr, coalesce(enabled, true) as enabled
             from script_binding
             where coalesce(enabled, true) = true
             order by kpz_id, grup, logical",
            &[],
        )
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(ScriptBindingRow {
            kpz_id: r.get::<_, i32>(0),
            grup: r.get::<_, i32>(1),
            logical: r.get::<_, i32>(2),
            reg_id: r.get::<_, Option<i32>>(3),
            addr: r.get::<_, Option<i32>>(4),
        });
    }
    Ok(out)
}

/// Загружает runtime-настройки планировщика из `scheduler_runtime_cfg`.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
///
/// # Returns
/// - `Ok(Some(SchedulerRuntimeCfg))`: найдена конфигурация.
/// - `Ok(None)`: таблица/строка отсутствует, нужно использовать дефолты.
/// - `Err(...)`: ошибка SQL, не покрытая fallback-логикой.
pub async fn load_scheduler_runtime_cfg(client: &Client) -> Result<Option<SchedulerRuntimeCfg>> {
    let row = match client
        .query_opt(
            "select
                coalesce(no_response_failures, 3)::int4 as no_response_failures,
                coalesce(no_response_backoff_sec, 600)::int8 as no_response_backoff_sec,
                coalesce(metrics_p95_warn_ms, 1000)::int8 as metrics_p95_warn_ms,
                coalesce(metrics_p95_crit_ms, 3000)::int8 as metrics_p95_crit_ms,
                coalesce(modbus_a_timeout_ms, 6000)::int8 as modbus_a_timeout_ms,
                coalesce(modbus_script_timeout_ms, 6000)::int8 as modbus_script_timeout_ms
             from scheduler_runtime_cfg
             order by id
             limit 1",
            &[],
        )
        .await
    {
        Ok(v) => v,
        Err(e) => {
            let undef_table = e
                .as_db_error()
                .map(|d| d.code().code() == "42P01")
                .unwrap_or(false);
            if undef_table {
                tracing::warn!("scheduler_runtime_cfg table not found; using defaults");
                return Ok(None);
            }
            let undef_column = e
                .as_db_error()
                .map(|d| d.code().code() == "42703")
                .unwrap_or(false);
            if undef_column {
                tracing::warn!(
                    "scheduler_runtime_cfg metrics columns not found; using p95 defaults"
                );
                let row_legacy = client
                    .query_opt(
                        "select
                            coalesce(no_response_failures, 3)::int4 as no_response_failures,
                            coalesce(no_response_backoff_sec, 600)::int8 as no_response_backoff_sec
                         from scheduler_runtime_cfg
                         order by id
                         limit 1",
                        &[],
                    )
                    .await?;
                let Some(r) = row_legacy else {
                    return Ok(None);
                };
                let fails: i32 = r.get(0);
                let backoff: i64 = r.get(1);
                return Ok(Some(SchedulerRuntimeCfg {
                    no_response_failures: fails.clamp(1, 20) as u8,
                    no_response_backoff_sec: backoff.clamp(1, 86_400) as u64,
                    metrics_p95_warn_ms: 1000,
                    metrics_p95_crit_ms: 3000,
                    modbus_a_timeout_ms: 6000,
                    modbus_script_timeout_ms: 6000,
                }));
            }
            return Err(e.into());
        }
    };

    let Some(r) = row else {
        return Ok(None);
    };
    let fails: i32 = r.get(0);
    let backoff: i64 = r.get(1);
    let p95_warn: i64 = r.get(2);
    let p95_crit: i64 = r.get(3);
    let mb_a_timeout: i64 = r.get(4);
    let mb_script_timeout: i64 = r.get(5);
    let p95_warn_ms = p95_warn.clamp(100, 60_000) as u64;
    let p95_crit_ms = p95_crit.clamp(p95_warn, 120_000) as u64;
    let modbus_a_timeout_ms = mb_a_timeout.clamp(200, 30_000) as u64;
    let modbus_script_timeout_ms = mb_script_timeout.clamp(200, 30_000) as u64;
    let cfg = SchedulerRuntimeCfg {
        no_response_failures: fails.clamp(1, 20) as u8,
        no_response_backoff_sec: backoff.clamp(1, 86_400) as u64,
        metrics_p95_warn_ms: p95_warn_ms,
        metrics_p95_crit_ms: p95_crit_ms,
        modbus_a_timeout_ms,
        modbus_script_timeout_ms,
    };
    Ok(Some(cfg))
}

/// Добавляет запись в `poll_log`.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
/// - `kpz_id`: идентификатор КПЗ (или `None` для общего события).
/// - `kind`: тип/категория события.
/// - `msg`: текст сообщения.
///
/// # Returns
/// - `Ok(())` при успешной вставке.
/// - `Err(...)` при ошибке SQL.
pub async fn insert_poll_log(
    client: &Client,
    kpz_id: Option<i32>,
    kind: &str,
    msg: &str,
) -> Result<()> {
    client
        .execute(
            "insert into poll_log(kpz_id, kind, msg) values($1, $2, $3)",
            &[&kpz_id, &kind, &msg],
        )
        .await?;
    Ok(())
}

/// Пакетно вставляет записи в `poll_log`.
pub async fn insert_poll_log_columns(
    client: &Client,
    kpz_id: &[i32],
    kpz_id_is_null: &[bool],
    kind: &[String],
    msg: &[String],
) -> Result<i64> {
    if kpz_id.is_empty() {
        return Ok(0);
    }

    let inserted = client
        .execute(
            "insert into poll_log(kpz_id, kind, msg) \
             select \
               case when u.kpz_id_is_null then null else u.kpz_id end, \
               u.kind, \
               u.msg \
             from unnest($1::int4[], $2::bool[], $3::text[], $4::text[]) \
             as u(kpz_id, kpz_id_is_null, kind, msg)",
            &[&kpz_id, &kpz_id_is_null, &kind, &msg],
        )
        .await?;

    Ok(inserted as i64)
}

/// Пакетно вставляет записи в `poll_log`.
#[cfg(test)]
#[allow(dead_code)]
pub async fn insert_poll_log_rows(
    client: &Client,
    rows: &[(Option<i32>, String, String)],
) -> Result<i64> {
    if rows.is_empty() {
        return Ok(0);
    }

    let mut kpz_id: Vec<i32> = Vec::with_capacity(rows.len());
    let mut kpz_id_is_null: Vec<bool> = Vec::with_capacity(rows.len());
    let mut kind: Vec<String> = Vec::with_capacity(rows.len());
    let mut msg: Vec<String> = Vec::with_capacity(rows.len());

    for (row_kpz_id, row_kind, row_msg) in rows {
        match row_kpz_id {
            Some(v) => {
                kpz_id.push(*v);
                kpz_id_is_null.push(false);
            }
            None => {
                kpz_id.push(0);
                kpz_id_is_null.push(true);
            }
        }
        kind.push(row_kind.clone());
        msg.push(row_msg.clone());
    }

    insert_poll_log_columns(client, &kpz_id, &kpz_id_is_null, &kind, &msg).await
}

/// Пакетно вставляет строки в `elam`.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
/// - `rows`: набор строк телеметрии ELAM для вставки.
///
/// # Returns
/// - `Ok(i64)`: число вставленных строк.
/// - `Err(...)`: ошибка SQL.
pub async fn insert_elam_rows(client: &Client, rows: &[ElamRow]) -> Result<i64> {
    if rows.is_empty() {
        return Ok(0);
    }

    let mut kpz_id: Vec<i32> = Vec::with_capacity(rows.len());
    let mut group_id: Vec<i32> = Vec::with_capacity(rows.len());
    let mut packet_id: Vec<i16> = Vec::with_capacity(rows.len());
    let mut pkt_type: Vec<i16> = Vec::with_capacity(rows.len());
    let mut func: Vec<i32> = Vec::with_capacity(rows.len());
    let mut addr_human: Vec<i32> = Vec::with_capacity(rows.len());
    let mut count_words: Vec<i32> = Vec::with_capacity(rows.len());
    let mut req: Vec<Vec<u8>> = Vec::with_capacity(rows.len());
    let mut resp: Vec<Vec<u8>> = Vec::with_capacity(rows.len());
    let mut resp_is_null: Vec<bool> = Vec::with_capacity(rows.len());
    let mut duration_ms: Vec<i32> = Vec::with_capacity(rows.len());
    let mut status: Vec<String> = Vec::with_capacity(rows.len());
    let mut err: Vec<String> = Vec::with_capacity(rows.len());
    let mut err_is_null: Vec<bool> = Vec::with_capacity(rows.len());

    for r in rows {
        kpz_id.push(r.kpz_id);
        group_id.push(r.group_id);
        packet_id.push(r.packet_id.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
        pkt_type.push(r.pkt_type.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
        func.push(r.func);
        addr_human.push(r.addr_human);
        count_words.push(r.count_words);
        req.push(r.req.clone());

        match &r.resp {
            Some(v) => {
                resp.push(v.clone());
                resp_is_null.push(false);
            }
            None => {
                resp.push(Vec::new());
                resp_is_null.push(true);
            }
        }

        duration_ms.push(r.duration_ms);
        status.push(r.status.clone());

        match &r.err {
            Some(v) => {
                err.push(v.clone());
                err_is_null.push(false);
            }
            None => {
                err.push(String::new());
                err_is_null.push(true);
            }
        }
    }

    let inserted = client
        .execute(
            "insert into elam(\
               kpz_id, group_id,\
               packet_id, pkt_type, func, addr_human, count_words,\
               req, resp, duration_ms, status, err\
             ) \
             select \
               u.kpz_id, u.group_id,\
               u.packet_id, u.pkt_type, u.func, u.addr_human, u.count_words,\
               u.req,\
               case when u.resp_is_null then null else u.resp end,\
               u.duration_ms, u.status,\
               case when u.err_is_null then null else u.err end \
             from unnest(\
               $1::int4[], $2::int4[],\
               $3::int2[], $4::int2[], $5::int4[], $6::int4[], $7::int4[],\
               $8::bytea[], $9::bytea[], $10::bool[],\
               $11::int4[], $12::text[], $13::text[], $14::bool[]\
             ) as u(\
               kpz_id, group_id,\
               packet_id, pkt_type, func, addr_human, count_words,\
               req, resp, resp_is_null,\
               duration_ms, status, err, err_is_null\
             )",
            &[
                &kpz_id,
                &group_id,
                &packet_id,
                &pkt_type,
                &func,
                &addr_human,
                &count_words,
                &req,
                &resp,
                &resp_is_null,
                &duration_ms,
                &status,
                &err,
                &err_is_null,
            ],
        )
        .await?;

    Ok(inserted as i64)
}

/// Удаляет порцию старых записей `elam` старше заданного числа дней.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
/// - `days`: возраст данных в днях.
/// - `batch_limit`: максимальное число удаляемых строк за вызов.
///
/// # Returns
/// - `Ok(i64)`: число удалённых строк.
/// - `Err(...)`: ошибка SQL.
pub async fn delete_elam_older_than_days_batch(
    client: &Client,
    days: i32,
    batch_limit: i64,
) -> Result<i64> {
    let res = client
        .execute(
            "with doomed as (\
               select ctid from elam \
               where ts < (now() - ($1::int || ' days')::interval) \
               order by ts \
               limit $2\
             ) \
             delete from elam e using doomed d where e.ctid = d.ctid",
            &[&days, &batch_limit],
        )
        .await?;
    Ok(res as i64)
}

/// Удаляет порцию старых записей `poll_log` старше заданного числа дней.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
/// - `days`: возраст данных в днях.
/// - `batch_limit`: максимальное число удаляемых строк за вызов.
///
/// # Returns
/// - `Ok(i64)`: число удалённых строк.
/// - `Err(...)`: ошибка SQL.
pub async fn delete_poll_log_older_than_days_batch(
    client: &Client,
    days: i32,
    batch_limit: i64,
) -> Result<i64> {
    let res = client
        .execute(
            "with doomed as (\
               select ctid from poll_log \
               where ts < (now() - ($1::int || ' days')::interval) \
               order by ts \
               limit $2\
             ) \
             delete from poll_log p using doomed d where p.ctid = d.ctid",
            &[&days, &batch_limit],
        )
        .await?;
    Ok(res as i64)
}

#[derive(Clone, Debug)]
/// Строка журнала `elam` для пакетной записи запроса/ответа Modbus.
pub struct ElamRow {
    pub kpz_id: i32,
    pub group_id: i32,
    pub packet_id: i32,
    pub pkt_type: i32,
    pub func: i32,
    pub addr_human: i32,
    pub count_words: i32,
    pub req: Vec<u8>,
    pub resp: Option<Vec<u8>>,
    pub duration_ms: i32,
    pub status: String,
    pub err: Option<String>,
}

/// Пакетно upsert-вставляет значения регистров в `arx_val`.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
/// - `rows`: строки значений для записи.
///
/// # Returns
/// - `Ok(i64)`: число вставленных/обновлённых строк по отчёту execute.
/// - `Err(...)`: ошибка SQL.
pub async fn insert_arx_val_rows(client: &Client, rows: &[ArxValRow]) -> Result<i64> {
    if rows.is_empty() {
        return Ok(0);
    }

    let mut kpz_id: Vec<i32> = Vec::with_capacity(rows.len());
    let mut reg_id: Vec<i32> = Vec::with_capacity(rows.len());
    let mut ts_unix: Vec<i64> = Vec::with_capacity(rows.len());
    let mut tip: Vec<i16> = Vec::with_capacity(rows.len());
    let mut val_num: Vec<f64> = Vec::with_capacity(rows.len());
    let mut val_raw: Vec<Vec<u8>> = Vec::with_capacity(rows.len());

    for r in rows {
        kpz_id.push(r.kpz_id);
        reg_id.push(r.reg_id);
        ts_unix.push(r.ts_unix);
        tip.push(r.tip.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
        val_num.push(r.val_num);
        val_raw.push(r.val_raw.clone());
    }

    let inserted = client
        .execute(
            "insert into arx_val(kpz_id, reg_id, ts_unix, tip, val_num, val_raw) \
             select u.kpz_id, u.reg_id, u.ts_unix, u.tip, u.val_num, u.val_raw \
             from unnest($1::int4[], $2::int4[], $3::int8[], $4::int2[], $5::float8[], $6::bytea[]) \
             as u(kpz_id, reg_id, ts_unix, tip, val_num, val_raw) \
             on conflict (kpz_id, reg_id, ts_unix) do update set \
               tip = excluded.tip, \
               val_num = excluded.val_num, \
               val_raw = excluded.val_raw",
            &[&kpz_id, &reg_id, &ts_unix, &tip, &val_num, &val_raw],
        )
        .await? as i64;

    let conflicts = rows.len() as i64 - inserted;
    debug!(
        attempted = rows.len(),
        inserted = inserted,
        conflicts = conflicts,
        "arx_val batch write finished"
    );
    Ok(inserted)
}

/// Загружает состояние индексов архива (`arx_state`) для заданного КПЗ.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
/// - `kpz_id`: идентификатор КПЗ.
///
/// # Returns
/// - `Ok(Vec<(i32, i32)>)`: пары `(arx_id, last_ind)`.
/// - `Err(...)`: ошибка SQL.
pub async fn load_arx_state_map(client: &Client, kpz_id: i32) -> Result<Vec<(i32, i32)>> {
    let rows = client
        .query(
            "select arx_id, last_ind from public.arx_state where kpz_id = $1",
            &[&kpz_id],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get::<_, i32>(0), r.get::<_, i32>(1)))
        .collect())
}

/// Обновляет `last_ind` в `arx_state` для пары `(kpz_id, arx_id)`.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
/// - `kpz_id`: идентификатор КПЗ.
/// - `arx_id`: идентификатор архивного источника.
/// - `last_ind`: новый индекс.
///
/// # Returns
/// - `Ok(())` при успешном upsert.
/// - `Err(...)` при ошибке SQL.
#[cfg(test)]
pub async fn set_arx_last_ind(
    client: &Client,
    kpz_id: i32,
    arx_id: i32,
    last_ind: i32,
) -> Result<()> {
    client
        .execute(
            "insert into public.arx_state(kpz_id, arx_id, last_ind) values($1, $2, $3) \
             on conflict (kpz_id, arx_id) do update set last_ind=excluded.last_ind, updated_at=now()",
            &[&kpz_id, &arx_id, &last_ind],
        )
        .await?;
    Ok(())
}

/// Пакетно upsert-обновляет `last_ind` в `arx_state`.
pub async fn set_arx_last_ind_columns(
    client: &Client,
    kpz_id: &[i32],
    arx_id: &[i32],
    last_ind: &[i32],
) -> Result<i64> {
    if kpz_id.is_empty() {
        return Ok(0);
    }

    let written = client
        .execute(
            "insert into public.arx_state(kpz_id, arx_id, last_ind) \
             select u.kpz_id, u.arx_id, u.last_ind \
             from unnest($1::int4[], $2::int4[], $3::int4[]) \
             as u(kpz_id, arx_id, last_ind) \
             on conflict (kpz_id, arx_id) do update set \
               last_ind = excluded.last_ind, \
               updated_at = now()",
            &[&kpz_id, &arx_id, &last_ind],
        )
        .await?;

    Ok(written as i64)
}

/// Пакетно upsert-обновляет `last_ind` в `arx_state`.
#[cfg(test)]
#[allow(dead_code)]
pub async fn set_arx_last_ind_rows(client: &Client, rows: &[(i32, i32, i32)]) -> Result<i64> {
    if rows.is_empty() {
        return Ok(0);
    }

    let mut kpz_id: Vec<i32> = Vec::with_capacity(rows.len());
    let mut arx_id: Vec<i32> = Vec::with_capacity(rows.len());
    let mut last_ind: Vec<i32> = Vec::with_capacity(rows.len());

    for (row_kpz_id, row_arx_id, row_last_ind) in rows {
        kpz_id.push(*row_kpz_id);
        arx_id.push(*row_arx_id);
        last_ind.push(*row_last_ind);
    }

    set_arx_last_ind_columns(client, &kpz_id, &arx_id, &last_ind).await
}

/// Проверяет наличие обязательных таблиц alarm-подсистемы.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
///
/// # Returns
/// - `Ok(true)`, если `alarm_rule`, `alarm_state`, `alarm_event` существуют.
/// - `Ok(false)`, если хотя бы одной таблицы нет.
/// - `Err(...)` при ошибке SQL.
pub async fn alarms_schema_present(client: &Client) -> Result<bool> {
    let row = client
        .query_one(
            "select \
               to_regclass('public.alarm_rule') is not null as rule_ok, \
               to_regclass('public.alarm_state') is not null as state_ok, \
               to_regclass('public.alarm_event') is not null as event_ok",
            &[],
        )
        .await?;
    let rule_ok: bool = row.get(0);
    let state_ok: bool = row.get(1);
    let event_ok: bool = row.get(2);
    Ok(rule_ok && state_ok && event_ok)
}

/// Загружает активные правила аварий из `alarm_rule`.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
///
/// # Returns
/// - `Ok(Vec<AlarmRule>)`: список правил.
/// - `Err(...)`: ошибка SQL/декодирования.
pub async fn load_alarm_rules(client: &Client) -> Result<Vec<AlarmRule>> {
    let rows = client
        .query(
            "select id, kpz_id, reg_id, cmp, set_lo, set_hi, set_lo_1, set_hi_1, \
                    coalesce(hysteresis, 0), coalesce(on_delay_sec, 0), coalesce(off_delay_sec, 0), \
                    coalesce(severity, 1), code, message \
             from public.alarm_rule \
             where enabled = true \
             order by kpz_id, reg_id, id",
            &[],
        )
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(AlarmRule {
            id: r.get::<_, i64>(0),
            kpz_id: r.get::<_, i32>(1),
            reg_id: r.get::<_, i32>(2),
            cmp: r
                .try_get::<_, String>(3)
                .ok()
                .unwrap_or_else(|| "gt".to_string()),
            set_lo: r.try_get::<_, Option<f64>>(4).ok().flatten(),
            set_hi: r.try_get::<_, Option<f64>>(5).ok().flatten(),
            set_lo_1: r.try_get::<_, Option<f64>>(6).ok().flatten(),
            set_hi_1: r.try_get::<_, Option<f64>>(7).ok().flatten(),
            hysteresis: r
                .try_get::<_, f64>(8)
                .ok()
                .or_else(|| r.try_get::<_, i32>(8).ok().map(|v| v as f64))
                .unwrap_or(0.0),
            on_delay_sec: r
                .try_get::<_, i32>(9)
                .ok()
                .or_else(|| r.try_get::<_, i16>(9).ok().map(|v| v as i32))
                .unwrap_or(0),
            off_delay_sec: r
                .try_get::<_, i32>(10)
                .ok()
                .or_else(|| r.try_get::<_, i16>(10).ok().map(|v| v as i32))
                .unwrap_or(0),
            severity: r
                .try_get::<_, i16>(11)
                .ok()
                .or_else(|| r.try_get::<_, i32>(11).ok().map(|v| v as i16))
                .unwrap_or(1),
            code: r.try_get::<_, Option<String>>(12).ok().flatten(),
            message: r.try_get::<_, Option<String>>(13).ok().flatten(),
        });
    }
    Ok(out)
}

/// Загружает последние значения `arx_val` по каждому `reg_id` для указанного КПЗ.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
/// - `kpz_id`: идентификатор КПЗ.
///
/// # Returns
/// - `Ok(Vec<(i32, f64)>)`: пары `(reg_id, val_num)`.
/// - `Err(...)`: ошибка SQL.
pub async fn load_latest_arx_val_map(client: &Client, kpz_id: i32) -> Result<Vec<(i32, f64)>> {
    let rows = client
        .query(
            "select distinct on (reg_id) reg_id, val_num \
             from public.arx_val \
             where kpz_id = $1 \
             order by reg_id, ts_unix desc",
            &[&kpz_id],
        )
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push((r.get::<_, i32>(0), r.get::<_, f64>(1)));
    }
    Ok(out)
}

/// Загружает текущее состояние alarm-правил (`rule_id -> active`).
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
///
/// # Returns
/// - `Ok(Vec<(i64, bool)>)`: карта состояний правил.
/// - `Err(...)`: ошибка SQL.
pub async fn load_alarm_state_map(client: &Client) -> Result<Vec<(i64, bool)>> {
    let rows = client
        .query("select rule_id, active from public.alarm_state", &[])
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get::<_, i64>(0), r.get::<_, bool>(1)))
        .collect())
}

/// Загружает маршруты Telegram-уведомлений для alarm-правил (`rule_id -> chat_id`)
/// из `alarm_rule.chat_id`.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
///
/// # Returns
/// - `Ok(Vec<(i64, String)>)`: пары `(rule_id, chat_id)` с включенными маршрутами.
/// - `Err(...)`: ошибка SQL (например, колонка `chat_id` отсутствует).
pub async fn load_alarm_notify_routes(client: &Client) -> Result<Vec<AlarmNotifyRoute>> {
    let rows_res = client
        .query(
            "select id as rule_id, chat_id, \
                    coalesce(tg_on_on, true) as tg_on_on, \
                    coalesce(tg_on_off, false) as tg_on_off, \
                    coalesce(tg_thr_main, true) as tg_thr_main, \
                    coalesce(tg_thr_lvl1, true) as tg_thr_lvl1 \
             from public.alarm_rule \
             where enabled = true and coalesce(trim(chat_id), '') <> ''",
            &[],
        )
        .await;

    let rows = match rows_res {
        Ok(v) => {
            return Ok(v
                .into_iter()
                .map(|r| AlarmNotifyRoute {
                    rule_id: r.get::<_, i64>(0),
                    chat_id: r.get::<_, String>(1),
                    on_on: r.get::<_, bool>(2),
                    on_off: r.get::<_, bool>(3),
                    thr_main: r.get::<_, bool>(4),
                    thr_lvl1: r.get::<_, bool>(5),
                })
                .collect())
        }
        Err(_) => {
            client
                .query(
                    "select id as rule_id, chat_id \
                 from public.alarm_rule \
                 where enabled = true and coalesce(trim(chat_id), '') <> ''",
                    &[],
                )
                .await?
        }
    };
    Ok(rows
        .into_iter()
        .map(|r| AlarmNotifyRoute {
            rule_id: r.get::<_, i64>(0),
            chat_id: r.get::<_, String>(1),
            on_on: true,
            on_off: false,
            thr_main: true,
            thr_lvl1: true,
        })
        .collect())
}

/// Upsert-обновляет состояние конкретного alarm-правила.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
/// - `rule_id`: идентификатор правила.
/// - `active`: новое состояние активности.
/// - `value`: последнее измеренное значение.
///
/// # Returns
/// - `Ok(())` при успешном upsert.
/// - `Err(...)` при ошибке SQL.
#[cfg(test)]
pub async fn upsert_alarm_state(
    client: &Client,
    rule_id: i64,
    active: bool,
    value: f64,
) -> Result<()> {
    client
        .execute(
            "insert into public.alarm_state(rule_id, active, active_since, last_value, updated_at) \
             values($1, $2, case when $2 then now() else null end, $3, now()) \
             on conflict (rule_id) do update set \
               active = excluded.active, \
               active_since = case when excluded.active then coalesce(public.alarm_state.active_since, now()) else null end, \
               last_value = excluded.last_value, \
               updated_at = now()",
            &[&rule_id, &active, &value],
        )
        .await?;
    Ok(())
}

/// Добавляет событие alarm-перехода в `alarm_event`.
///
/// # Parameters
/// - `client`: клиент PostgreSQL.
/// - `kpz_id`, `reg_id`, `rule_id`: идентификаторы источника события.
/// - `event`: тип события (`on`/`off` и т.п.).
/// - `value`: зафиксированное значение.
/// - `set_lo`, `set_hi`: уставки на момент события.
/// - `severity`: уровень серьёзности.
/// - `code`, `message`: дополнительные коды/описания.
///
/// # Returns
/// - `Ok(())` при успешной вставке.
/// - `Err(...)` при ошибке SQL.
#[cfg(test)]
pub async fn insert_alarm_event(
    client: &Client,
    kpz_id: i32,
    reg_id: i32,
    rule_id: i64,
    event: &str,
    value: f64,
    set_lo: Option<f64>,
    set_hi: Option<f64>,
    severity: i16,
    code: Option<&str>,
    message: Option<&str>,
) -> Result<()> {
    client
        .execute(
            "insert into public.alarm_event(\
                kpz_id, reg_id, rule_id, event, value, set_lo, set_hi, severity, code, message\
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            &[
                &kpz_id, &reg_id, &rule_id, &event, &value, &set_lo, &set_hi, &severity, &code,
                &message,
            ],
        )
        .await?;
    Ok(())
}

/// Пакетно upsert-обновляет состояние alarm-правил.
pub async fn upsert_alarm_state_columns(
    client: &Client,
    rule_id: &[i64],
    active: &[bool],
    value: &[f64],
) -> Result<i64> {
    if rule_id.is_empty() {
        return Ok(0);
    }

    let written = client
        .execute(
            "insert into public.alarm_state(rule_id, active, active_since, last_value, updated_at) \
             select \
               u.rule_id, \
               u.active, \
               case when u.active then now() else null end, \
               u.last_value, \
               now() \
             from unnest($1::int8[], $2::bool[], $3::float8[]) \
             as u(rule_id, active, last_value) \
             on conflict (rule_id) do update set \
               active = excluded.active, \
               active_since = case when excluded.active then coalesce(public.alarm_state.active_since, now()) else null end, \
               last_value = excluded.last_value, \
               updated_at = now()",
            &[&rule_id, &active, &value],
        )
        .await?;

    Ok(written as i64)
}

/// Пакетно upsert-обновляет состояние alarm-правил.
#[cfg(test)]
#[allow(dead_code)]
pub async fn upsert_alarm_state_rows(client: &Client, rows: &[(i64, bool, f64)]) -> Result<i64> {
    if rows.is_empty() {
        return Ok(0);
    }

    let mut rule_id: Vec<i64> = Vec::with_capacity(rows.len());
    let mut active: Vec<bool> = Vec::with_capacity(rows.len());
    let mut value: Vec<f64> = Vec::with_capacity(rows.len());

    for (row_rule_id, row_active, row_value) in rows {
        rule_id.push(*row_rule_id);
        active.push(*row_active);
        value.push(*row_value);
    }

    upsert_alarm_state_columns(client, &rule_id, &active, &value).await
}

/// Пакетно вставляет события alarm-переходов в `alarm_event`.
#[allow(clippy::too_many_arguments)]
pub async fn insert_alarm_event_columns(
    client: &Client,
    kpz_id: &[i32],
    reg_id: &[i32],
    rule_id: &[i64],
    event: &[String],
    value: &[f64],
    set_lo: &[f64],
    set_lo_is_null: &[bool],
    set_hi: &[f64],
    set_hi_is_null: &[bool],
    severity: &[i16],
    code: &[String],
    code_is_null: &[bool],
    message: &[String],
    message_is_null: &[bool],
) -> Result<i64> {
    if kpz_id.is_empty() {
        return Ok(0);
    }

    let inserted = client
        .execute(
            "insert into public.alarm_event(\
                kpz_id, reg_id, rule_id, event, value, set_lo, set_hi, severity, code, message\
            ) \
            select \
                u.kpz_id, u.reg_id, u.rule_id, u.event, u.value, \
                case when u.set_lo_is_null then null else u.set_lo end, \
                case when u.set_hi_is_null then null else u.set_hi end, \
                u.severity, \
                case when u.code_is_null then null else u.code end, \
                case when u.message_is_null then null else u.message end \
            from unnest(\
                $1::int4[], $2::int4[], $3::int8[], $4::text[], $5::float8[], \
                $6::float8[], $7::bool[], $8::float8[], $9::bool[], $10::int2[], \
                $11::text[], $12::bool[], $13::text[], $14::bool[]\
            ) as u(\
                kpz_id, reg_id, rule_id, event, value, \
                set_lo, set_lo_is_null, set_hi, set_hi_is_null, severity, \
                code, code_is_null, message, message_is_null\
            )",
            &[
                &kpz_id,
                &reg_id,
                &rule_id,
                &event,
                &value,
                &set_lo,
                &set_lo_is_null,
                &set_hi,
                &set_hi_is_null,
                &severity,
                &code,
                &code_is_null,
                &message,
                &message_is_null,
            ],
        )
        .await?;

    Ok(inserted as i64)
}

/// Пакетно вставляет события alarm-переходов в `alarm_event`.
#[cfg(test)]
#[allow(dead_code)]
pub async fn insert_alarm_event_rows(
    client: &Client,
    rows: &[(
        i32,
        i32,
        i64,
        String,
        f64,
        Option<f64>,
        Option<f64>,
        i16,
        Option<String>,
        Option<String>,
    )],
) -> Result<i64> {
    if rows.is_empty() {
        return Ok(0);
    }

    let mut kpz_id: Vec<i32> = Vec::with_capacity(rows.len());
    let mut reg_id: Vec<i32> = Vec::with_capacity(rows.len());
    let mut rule_id: Vec<i64> = Vec::with_capacity(rows.len());
    let mut event: Vec<String> = Vec::with_capacity(rows.len());
    let mut value: Vec<f64> = Vec::with_capacity(rows.len());
    let mut set_lo: Vec<f64> = Vec::with_capacity(rows.len());
    let mut set_lo_is_null: Vec<bool> = Vec::with_capacity(rows.len());
    let mut set_hi: Vec<f64> = Vec::with_capacity(rows.len());
    let mut set_hi_is_null: Vec<bool> = Vec::with_capacity(rows.len());
    let mut severity: Vec<i16> = Vec::with_capacity(rows.len());
    let mut code: Vec<String> = Vec::with_capacity(rows.len());
    let mut code_is_null: Vec<bool> = Vec::with_capacity(rows.len());
    let mut message: Vec<String> = Vec::with_capacity(rows.len());
    let mut message_is_null: Vec<bool> = Vec::with_capacity(rows.len());

    for row in rows {
        kpz_id.push(row.0);
        reg_id.push(row.1);
        rule_id.push(row.2);
        event.push(row.3.clone());
        value.push(row.4);

        match row.5 {
            Some(v) => {
                set_lo.push(v);
                set_lo_is_null.push(false);
            }
            None => {
                set_lo.push(0.0);
                set_lo_is_null.push(true);
            }
        }

        match row.6 {
            Some(v) => {
                set_hi.push(v);
                set_hi_is_null.push(false);
            }
            None => {
                set_hi.push(0.0);
                set_hi_is_null.push(true);
            }
        }

        severity.push(row.7);

        match &row.8 {
            Some(v) => {
                code.push(v.clone());
                code_is_null.push(false);
            }
            None => {
                code.push(String::new());
                code_is_null.push(true);
            }
        }

        match &row.9 {
            Some(v) => {
                message.push(v.clone());
                message_is_null.push(false);
            }
            None => {
                message.push(String::new());
                message_is_null.push(true);
            }
        }
    }

    insert_alarm_event_columns(
        client,
        &kpz_id,
        &reg_id,
        &rule_id,
        &event,
        &value,
        &set_lo,
        &set_lo_is_null,
        &set_hi,
        &set_hi_is_null,
        &severity,
        &code,
        &code_is_null,
        &message,
        &message_is_null,
    )
    .await
}

/// Строит нормализованный `ConnInfo` из `kpz` и связанных справочников `obj/ip/port`.
///
/// # Parameters
/// - `kpz`: строка КПЗ.
/// - `obj_by_id`: словарь объектов связи.
/// - `ip_by_id`: словарь `ip`-справочника.
/// - `port_by_id`: словарь `port`-справочника.
///
/// # Returns
/// - `Ok(ConnInfo)`: готовые реквизиты соединения.
/// - `Err(...)`: отсутствует объект/справочник или невалидны `ip/port`.
pub fn build_conn(
    kpz: &KpzRow,
    obj_by_id: &std::collections::HashMap<i32, ObjRow>,
    ip_by_id: &std::collections::HashMap<i32, String>,
    port_by_id: &std::collections::HashMap<i32, u16>,
) -> Result<ConnInfo> {
    let obj = obj_by_id
        .get(&kpz.obj)
        .ok_or_else(|| anyhow::anyhow!("obj={} not found", kpz.obj))?;

    let ip_field = obj
        .ip
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("obj.id={} has empty ip field", obj.id))?;
    let ip = if let Ok(ip_id) = ip_field.parse::<i32>() {
        ip_by_id.get(&ip_id).cloned().ok_or_else(|| {
            anyhow::anyhow!("ip id={} referenced by obj.id={} is missing", ip_id, obj.id)
        })?
    } else {
        ip_field
    };

    let port_field = obj
        .port
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("obj.id={} has empty port field", obj.id))?;
    let port = if let Ok(port_id) = port_field.parse::<i32>() {
        port_by_id.get(&port_id).copied().ok_or_else(|| {
            anyhow::anyhow!(
                "port id={} referenced by obj.id={} is missing",
                port_id,
                obj.id
            )
        })?
    } else {
        port_field.parse::<u16>().map_err(|e| {
            anyhow::anyhow!("invalid port='{}' for obj.id={}: {}", port_field, obj.id, e)
        })?
    };

    Ok(ConnInfo {
        kpz_id: kpz.id,
        obj_id: obj.id,
        ip,
        port,
        rtu: kpz.rtu,
        modem: kpz.modem,
        max_pkt_len: kpz.max_pkt_len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio_postgres::NoTls;

    async fn connect_test_db() -> Result<Client> {
        let url = env::var("TEST_DB_URL")
            .map_err(|_| anyhow::anyhow!("TEST_DB_URL is required for db integration tests"))?;

        let (client, connection) = tokio_postgres::connect(&url, NoTls).await?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("db connection task error: {e}");
            }
        });
        Ok(client)
    }

    #[tokio::test]
    #[ignore = "requires TEST_DB_URL"]
    async fn db_integration_alarm_and_arx_val_roundtrip() -> Result<()> {
        let client = connect_test_db().await?;

        client.execute("begin", &[]).await?;

        let maybe_rule = client
            .query_opt(
                "select id, kpz_id, reg_id \
                 from public.alarm_rule \
                 where enabled = true \
                 order by id \
                 limit 1",
                &[],
            )
            .await?;
        let Some(rule_row) = maybe_rule else {
            eprintln!("skip db integration test: no enabled rows in public.alarm_rule");
            client.execute("rollback", &[]).await?;
            return Ok(());
        };
        let rule_id: i64 = rule_row.get(0);
        let kpz_id: i32 = rule_row.get(1);
        let reg_id: i32 = rule_row.get(2);

        // 1) arx_val conflict update: second write must win for the same second key.
        let ts_unix = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64)
            + 7;
        let row1 = ArxValRow {
            kpz_id,
            reg_id,
            ts_unix,
            tip: 5,
            val_num: 11.0,
            val_raw: (11.0f32).to_be_bytes().to_vec(),
        };
        let row2 = ArxValRow {
            kpz_id,
            reg_id,
            ts_unix,
            tip: 5,
            val_num: 99.5,
            val_raw: (99.5f32).to_be_bytes().to_vec(),
        };

        let _ = insert_arx_val_rows(&client, &[row1]).await?;
        let _ = insert_arx_val_rows(&client, &[row2]).await?;

        let arx = client
            .query_one(
                "select val_num from public.arx_val \
                 where kpz_id=$1 and reg_id=$2 and ts_unix=$3",
                &[&kpz_id, &reg_id, &ts_unix],
            )
            .await?;
        let final_val: f64 = arx.get(0);
        assert!(
            (final_val - 99.5).abs() < 1e-9,
            "expected arx_val upsert to keep the latest value"
        );

        // 2) alarm_state + alarm_event write path.
        upsert_alarm_state(&client, rule_id, true, 123.45).await?;
        let st = client
            .query_one(
                "select active, last_value from public.alarm_state where rule_id=$1",
                &[&rule_id],
            )
            .await?;
        let active: bool = st.get(0);
        let last_value: f64 = st.get(1);
        assert!(active, "alarm_state.active should be true");
        assert!(
            (last_value - 123.45).abs() < 1e-9,
            "alarm_state.last_value mismatch"
        );

        let marker = format!("itest-{}", ts_unix);
        insert_alarm_event(
            &client,
            kpz_id,
            reg_id,
            rule_id,
            "on",
            123.45,
            Some(0.0),
            Some(45.0),
            1,
            Some("itest"),
            Some(&marker),
        )
        .await?;
        let ev = client
            .query_one(
                "select event, value, message from public.alarm_event \
                 where rule_id=$1 and message=$2 \
                 order by ts desc \
                 limit 1",
                &[&rule_id, &marker],
            )
            .await?;
        let ev_name: String = ev.get(0);
        let ev_value: f64 = ev.get(1);
        let ev_message: Option<String> = ev.get(2);
        assert_eq!(ev_name, "on");
        assert!(
            (ev_value - 123.45).abs() < 1e-9,
            "alarm_event.value mismatch"
        );
        assert_eq!(ev_message.as_deref(), Some(marker.as_str()));

        client.execute("rollback", &[]).await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires TEST_DB_URL"]
    async fn db_integration_specific_rule_kpz5_reg6002_rule1() -> Result<()> {
        let client = connect_test_db().await?;
        client.execute("begin", &[]).await?;

        let rule = client
            .query_opt(
                "select id, kpz_id, reg_id, enabled \
                 from public.alarm_rule \
                 where id = 1 and kpz_id = 5 and reg_id = 6002",
                &[],
            )
            .await?;
        let Some(rule) = rule else {
            eprintln!(
                "skip specific db integration test: alarm_rule(id=1,kpz=5,reg=6002) not found"
            );
            client.execute("rollback", &[]).await?;
            return Ok(());
        };
        let rule_id: i64 = rule.get(0);
        let kpz_id: i32 = rule.get(1);
        let reg_id: i32 = rule.get(2);
        let enabled: bool = rule.get(3);
        assert!(enabled, "alarm_rule(1) must be enabled for this scenario");

        // Check alarm_state upsert for this exact rule.
        upsert_alarm_state(&client, rule_id, true, 88.0).await?;
        let st = client
            .query_one(
                "select active, last_value from public.alarm_state where rule_id=$1",
                &[&rule_id],
            )
            .await?;
        let active: bool = st.get(0);
        let last_value: f64 = st.get(1);
        assert!(active);
        assert!((last_value - 88.0).abs() < 1e-9);

        // Check alarm_event insert for the same tuple.
        let ts_marker = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64)
            + 13;
        let marker = format!("itest-kpz5-{}", ts_marker);
        insert_alarm_event(
            &client,
            kpz_id,
            reg_id,
            rule_id,
            "on",
            88.0,
            Some(0.0),
            Some(45.0),
            1,
            Some("itest-kpz5"),
            Some(&marker),
        )
        .await?;
        let ev = client
            .query_one(
                "select kpz_id, reg_id, rule_id, event, value, message \
                 from public.alarm_event \
                 where rule_id=$1 and message=$2 \
                 order by ts desc \
                 limit 1",
                &[&rule_id, &marker],
            )
            .await?;
        let ev_kpz: i32 = ev.get(0);
        let ev_reg: i32 = ev.get(1);
        let ev_rule: i64 = ev.get(2);
        let ev_name: String = ev.get(3);
        let ev_value: f64 = ev.get(4);
        let ev_message: Option<String> = ev.get(5);
        assert_eq!(ev_kpz, 5);
        assert_eq!(ev_reg, 6002);
        assert_eq!(ev_rule, 1);
        assert_eq!(ev_name, "on");
        assert!((ev_value - 88.0).abs() < 1e-9);
        assert_eq!(ev_message.as_deref(), Some(marker.as_str()));

        client.execute("rollback", &[]).await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires TEST_DB_URL"]
    async fn db_integration_obj_fingerprint_query_accepts_integer_port_column() -> Result<()> {
        let client = connect_test_db().await?;

        let row = client
            .query_one(
                "select
                    count(*)::int8,
                    coalesce(max(id), 0)::int8,
                    coalesce(sum(
                        id::int8 +
                        coalesce(kanal, 0)::int8 + coalesce(speed, 0)::int8 +
                        coalesce(stop, 0)::int8 + coalesce(parit, 0)::int8 +
                        coalesce(bit, 0)::int8 +
                        length(coalesce(ip, ''))::int8 +
                        length(coalesce(port::text, ''))::int8
                    ), 0)::int8
                 from obj",
                &[],
            )
            .await?;

        let _count: i64 = row.get(0);
        let _max_id: i64 = row.get(1);
        let _sum_sig: i64 = row.get(2);
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires TEST_DB_URL"]
    async fn db_integration_load_topology_fingerprint_succeeds() -> Result<()> {
        let client = connect_test_db().await?;
        let fp = load_topology_fingerprint(&client).await?;

        let _any_nonzero = fp.kpz_sig != 0
            || fp.obj_sig != 0
            || fp.ip_sig != 0
            || fp.port_sig != 0
            || fp.n_mb_sig != 0
            || fp.reg_sig != 0
            || fp.g_script_sig != 0
            || fp.binding_sig != 0;
        Ok(())
    }

    fn sample_kpz(obj: i32) -> KpzRow {
        KpzRow {
            id: 77,
            name: Some("kpz-test".to_string()),
            rtu: 3,
            obj,
            modem: 9,
            grups: vec![0u8; 64],
            max_pkt_len: 512,
            start: 1,
            t_a: 1,
            t_script: 1,
            en_post: true,
        }
    }

    #[test]
    fn build_conn_resolves_ip_and_port_lookup_ids() {
        let kpz = sample_kpz(10);
        let mut obj_by_id = HashMap::new();
        obj_by_id.insert(
            10,
            ObjRow {
                id: 10,
                name: Some("obj-10".to_string()),
                ip: Some("5".to_string()),
                port: Some("7".to_string()),
                kanal: None,
                speed: None,
                stop: None,
                parit: None,
                bit: None,
            },
        );

        let mut ip_by_id = HashMap::new();
        ip_by_id.insert(5, "192.168.1.55".to_string());

        let mut port_by_id = HashMap::new();
        port_by_id.insert(7, 4050u16);

        let conn = build_conn(&kpz, &obj_by_id, &ip_by_id, &port_by_id).expect("conn");
        assert_eq!(conn.kpz_id, 77);
        assert_eq!(conn.obj_id, 10);
        assert_eq!(conn.ip, "192.168.1.55");
        assert_eq!(conn.port, 4050);
        assert_eq!(conn.rtu, 3);
        assert_eq!(conn.modem, 9);
        assert_eq!(conn.max_pkt_len, 512);
    }

    #[test]
    fn build_conn_accepts_direct_ip_and_lookup_port() {
        let kpz = sample_kpz(11);
        let mut obj_by_id = HashMap::new();
        obj_by_id.insert(
            11,
            ObjRow {
                id: 11,
                name: Some("obj-11".to_string()),
                ip: Some("10.0.0.7".to_string()),
                port: Some("8".to_string()),
                kanal: None,
                speed: None,
                stop: None,
                parit: None,
                bit: None,
            },
        );
        let mut port_by_id = HashMap::new();
        port_by_id.insert(8, 502u16);

        let conn = build_conn(&kpz, &obj_by_id, &HashMap::new(), &port_by_id).expect("conn");
        assert_eq!(conn.ip, "10.0.0.7");
        assert_eq!(conn.port, 502);
    }

    #[test]
    fn build_conn_rejects_empty_ip_field() {
        let kpz = sample_kpz(12);
        let mut obj_by_id = HashMap::new();
        obj_by_id.insert(
            12,
            ObjRow {
                id: 12,
                name: Some("obj-12".to_string()),
                ip: Some("   ".to_string()),
                port: Some("502".to_string()),
                kanal: None,
                speed: None,
                stop: None,
                parit: None,
                bit: None,
            },
        );

        let err = build_conn(&kpz, &obj_by_id, &HashMap::new(), &HashMap::new()).unwrap_err();
        assert!(
            err.to_string().contains("empty ip field"),
            "unexpected err: {err}"
        );
    }

    #[test]
    fn build_conn_rejects_missing_port_lookup() {
        let kpz = sample_kpz(13);
        let mut obj_by_id = HashMap::new();
        obj_by_id.insert(
            13,
            ObjRow {
                id: 13,
                name: Some("obj-13".to_string()),
                ip: Some("127.0.0.1".to_string()),
                port: Some("99".to_string()),
                kanal: None,
                speed: None,
                stop: None,
                parit: None,
                bit: None,
            },
        );

        let err = build_conn(&kpz, &obj_by_id, &HashMap::new(), &HashMap::new()).unwrap_err();
        assert!(
            err.to_string().contains("port id=99"),
            "unexpected err: {err}"
        );
    }
}
