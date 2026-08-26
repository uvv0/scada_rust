use std::{env, fs, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::runtime::Runtime;
use tokio_postgres::{Client, Config, NoTls};

use crate::models::{
    AlarmEventRow, AlarmRuleRow, AlarmStateRow, ArxPointRow, ArxSeriesRow, ArxStateRow, ArxValViewRow, DictItemRow, ElamRow,
    GScriptRow, GScriptTemplateRow, GroupRow, KpzIoRow, KpzRow, ObjRow, PollLogRow, RegEditRow,
    RegRow, SchedulerRuntimeCfgRow,
    UiKpzWindowRow, UiWindowBindingRow, UiWindowGroupRow,
};

const PG_HOST: &str = "localhost";
const PG_PORT: u16 = 5432;
const PG_DB: &str = "postgres_restored";
const PG_USER: &str = "uvv0";
const PG_PASS: &str = "z";

#[derive(Default, Deserialize)]
struct DbTomlConfig {
    db: Option<DbConfigSection>,
    pg_host: Option<String>,
    pg_port: Option<u16>,
    pg_db: Option<String>,
    pg_user: Option<String>,
    pg_pass: Option<String>,
}

#[derive(Default, Deserialize)]
struct DbConfigSection {
    host: Option<String>,
    port: Option<u16>,
    db: Option<String>,
    user: Option<String>,
    pass: Option<String>,
}

pub struct Db {
    rt: Runtime,
    client: Client,
}

impl Db {
    fn load_db_config() -> DbConfigSection {
        let path = env::current_exe()
            .ok()
            .map(|mut p| {
                p.set_file_name("ss5.toml");
                p
            })
            .unwrap_or_else(|| PathBuf::from("ss5.toml"));

        let file_cfg = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| toml::from_str::<DbTomlConfig>(&raw).ok())
            .map(|cfg| {
                cfg.db.unwrap_or(DbConfigSection {
                    host: cfg.pg_host,
                    port: cfg.pg_port,
                    db: cfg.pg_db,
                    user: cfg.pg_user,
                    pass: cfg.pg_pass,
                })
            })
            .unwrap_or_default();

        DbConfigSection {
            host: env::var("PG_HOST").ok().or(file_cfg.host),
            port: env::var("PG_PORT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .or(file_cfg.port),
            db: env::var("PG_DB").ok().or(file_cfg.db),
            user: env::var("PG_USER").ok().or(file_cfg.user),
            pass: env::var("PG_PASS").ok().or(file_cfg.pass),
        }
    }

    /// Function: $name.
    pub fn connect_from_env() -> Result<Self> {
        let cfg = Self::load_db_config();
        let host = cfg.host.unwrap_or_else(|| PG_HOST.to_string());
        let port = cfg.port.unwrap_or(PG_PORT);
        let db = cfg.db.unwrap_or_else(|| PG_DB.to_string());
        let user = cfg.user.unwrap_or_else(|| PG_USER.to_string());
        let pass = cfg.pass.unwrap_or_else(|| PG_PASS.to_string());

        let rt = Runtime::new()?;
        let mut pg_cfg = Config::new();
        pg_cfg
            .host(&host)
            .port(port)
            .user(&user)
            .password(&pass)
            .dbname(&db)
            .connect_timeout(Duration::from_secs(5));
        let client = rt
            .block_on(async {
            let primary = pg_cfg.connect(NoTls).await;
            let (client, conn) = match primary {
                Ok(v) => v,
                Err(first_err)
                    if !host.eq_ignore_ascii_case("localhost") && host != "127.0.0.1" =>
                {
                    let first_msg = first_err.to_string();
                    let mut fallback_cfg = Config::new();
                    fallback_cfg
                        .host("localhost")
                        .port(port)
                        .user(&user)
                        .password(&pass)
                        .dbname(&db)
                        .connect_timeout(Duration::from_secs(3));
                    fallback_cfg.connect(NoTls).await.with_context(|| {
                        format!(
                            "primary db host {}:{} failed ({}); fallback localhost:{} failed",
                            host, port, first_msg, port
                        )
                    })?
                }
                Err(e) => return Err(e.into()),
            };
            tokio::spawn(async move {
                if let Err(e) = conn.await {
                    eprintln!("db connection error: {e}");
                }
            });
            // Keep schema compatible with KPZ editor fields.
            client
                .execute(
                    "alter table if exists public.kpz add column if not exists en_post boolean not null default false",
                    &[],
                )
                .await?;
            client
                .batch_execute(
                    "create schema if not exists ui;

                     create table if not exists ui.kpz_window (
                         id bigserial primary key,
                         kpz_id int not null references public.kpz(id) on delete cascade,
                         code text not null,
                         title text not null,
                         description text,
                         is_active boolean not null default true,
                         updated_at timestamptz not null default now(),
                         unique (kpz_id, code)
                     );

                     create index if not exists idx_kpz_window_kpz
                         on ui.kpz_window (kpz_id);

                     create table if not exists ui.kpz_window_group (
                         window_id bigint not null references ui.kpz_window(id) on delete cascade,
                         group_id int not null,
                         pos int not null default 0,
                         updated_at timestamptz not null default now(),
                         primary key (window_id, group_id),
                         unique (window_id, pos)
                     );

                     create table if not exists ui.kpz_window_reg_binding (
                         window_id bigint not null references ui.kpz_window(id) on delete cascade,
                         reg_id int not null references public.reg(id) on delete restrict,
                         pos int not null default 0,
                         x int not null default 20,
                         y int not null default 20,
                         w int not null default 120,
                         h int not null default 34,
                         visible boolean not null default true,
                         writable boolean not null default false,
                         label_override text,
                         unit text,
                         fmt text,
                         updated_at timestamptz not null default now(),
                         primary key (window_id, reg_id),
                         unique (window_id, pos)
                     );

                     create table if not exists ui.gscript_template (
                         id bigserial primary key,
                         name text not null unique,
                         pre_src text not null default '',
                         post_src text not null default '',
                         elam smallint not null default 0,
                         max_words int not null default 800,
                         max_k int not null default 2,
                         en boolean not null default true,
                         ver int not null default 1,
                         updated_at timestamptz not null default now()
                     );

                     create table if not exists ui.gscript_group_template (
                         group_id int primary key,
                         template_id bigint not null references ui.gscript_template(id) on delete cascade,
                         updated_at timestamptz not null default now()
                     );

                     create table if not exists public.scheduler_runtime_cfg (
                         id bigserial primary key,
                         no_response_failures integer not null default 3,
                         no_response_backoff_sec bigint not null default 600,
                         metrics_p95_warn_ms bigint not null default 1000,
                         metrics_p95_crit_ms bigint not null default 3000,
                         modbus_a_timeout_ms bigint not null default 1800,
                         modbus_script_timeout_ms bigint not null default 2600,
                         updated_at timestamptz not null default now()
                     );
                     alter table public.scheduler_runtime_cfg add column if not exists metrics_p95_warn_ms bigint not null default 1000;
                     alter table public.scheduler_runtime_cfg add column if not exists metrics_p95_crit_ms bigint not null default 3000;
                     alter table public.scheduler_runtime_cfg add column if not exists modbus_a_timeout_ms bigint not null default 1800;
                     alter table public.scheduler_runtime_cfg add column if not exists modbus_script_timeout_ms bigint not null default 2600;
                     alter table if exists ui.kpz_window_reg_binding add column if not exists x int not null default 20;
                     alter table if exists ui.kpz_window_reg_binding add column if not exists y int not null default 20;
                     alter table if exists ui.kpz_window_reg_binding add column if not exists w int not null default 120;
                     alter table if exists ui.kpz_window_reg_binding add column if not exists h int not null default 34;
                     with ranked as (
                         select
                             window_id,
                             reg_id,
                             row_number() over (partition by window_id order by pos, reg_id) - 1 as idx
                         from ui.kpz_window_reg_binding
                         where x = 20 and y = 20 and w = 120 and h = 34
                     )
                     update ui.kpz_window_reg_binding b
                     set x = 20 + ((ranked.idx % 4) * 130),
                         y = 20 + ((ranked.idx / 4) * 52)
                     from ranked
                     where b.window_id = ranked.window_id and b.reg_id = ranked.reg_id;",
                )
                .await?;
            client
                .execute(
                    "insert into public.scheduler_runtime_cfg(id, no_response_failures, no_response_backoff_sec, metrics_p95_warn_ms, metrics_p95_crit_ms, modbus_a_timeout_ms, modbus_script_timeout_ms) \
                     values (1, 3, 600, 1000, 3000, 1800, 2600) \
                     on conflict (id) do nothing",
                    &[],
                )
                .await?;
            Result::<Client>::Ok(client)
        })
            .context("db initialization failed")?;

        Ok(Self { rt, client })
    }

    /// Function: $name.
    pub fn get_all_kpz(&self) -> Result<Vec<KpzRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, coalesce(name,''), coalesce(rtu,0), coalesce(obj,0), modem, \
                     grups, max_pkt_len, coalesce(start,0), coalesce(t_a::text,''), \
                     coalesce(t_script::text,''), coalesce(en_post, false) \
                     from kpz order by id",
                    &[],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| {
                    let grups = r
                        .try_get::<_, Option<Vec<u8>>>(5)
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| vec![0u8; 64]);
                    KpzRow {
                        id: r.get::<_, i32>(0),
                        name: r.get::<_, String>(1),
                        rtu: r.get::<_, i32>(2),
                        obj: r.get::<_, i32>(3),
                        modem: r.try_get::<_, Option<i32>>(4).ok().flatten(),
                        max_pkt_len: r.try_get::<_, Option<i32>>(6).ok().flatten(),
                        start: r.get::<_, i32>(7),
                        grups,
                        t_a: r.get::<_, String>(8),
                        t_script: r.get::<_, String>(9),
                        en_post: r.get::<_, bool>(10),
                    }
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn get_all_obj(&self) -> Result<Vec<ObjRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, coalesce(name,''), ip, port, kanal, speed, stop, parit, bit \
                     from obj order by id",
                    &[],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| {
                    let ip_raw = r.try_get::<_, Option<String>>(2).ok().flatten();
                    let ip = ip_raw
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .and_then(|s| s.parse::<i32>().ok());
                    ObjRow {
                        id: r.get::<_, i32>(0),
                        name: r.get::<_, String>(1),
                        ip_raw,
                        ip,
                        port: r.try_get::<_, Option<i32>>(3).ok().flatten(),
                        kanal: r.try_get::<_, Option<i32>>(4).ok().flatten(),
                        speed: r.try_get::<_, Option<i32>>(5).ok().flatten(),
                        stop: r.try_get::<_, Option<i32>>(6).ok().flatten(),
                        parit: r.try_get::<_, Option<i32>>(7).ok().flatten(),
                        bit: r.try_get::<_, Option<i32>>(8).ok().flatten(),
                    }
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn get_items(&self, table: &str) -> Result<Vec<DictItemRow>> {
        let sql = match table {
            "ip" | "port" | "speed" | "parit" | "bit" | "stop" | "kanal" | "grup" | "n_mb"
            | "tip" | "bits" | "c" => {
                format!("select id, coalesce(name,'') from {} order by id", table)
            }
            _ => return Err(anyhow::anyhow!("table not allowed: {table}")),
        };
        self.rt.block_on(async {
            let rows = self.client.query(&sql, &[]).await?;
            let out = rows
                .into_iter()
                .map(|r| DictItemRow {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn upsert_item(&self, table: &str, id: i32, name: &str) -> Result<()> {
        let sql = match table {
            "ip" | "port" | "speed" | "parit" | "bit" | "stop" | "kanal" | "grup" | "n_mb"
            | "tip" | "bits" | "c" => {
                format!(
                    "insert into {}(id, name) values($1, $2) \
                     on conflict (id) do update set name = excluded.name",
                    table
                )
            }
            _ => return Err(anyhow::anyhow!("table not allowed: {table}")),
        };
        self.rt.block_on(async {
            self.client.execute(&sql, &[&id, &name]).await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn delete_item(&self, table: &str, id: i32) -> Result<()> {
        let sql = match table {
            "ip" | "port" | "speed" | "parit" | "bit" | "stop" | "kanal" | "grup" | "n_mb"
            | "tip" | "bits" | "c" => {
                format!("delete from {} where id = $1", table)
            }
            _ => return Err(anyhow::anyhow!("table not allowed: {table}")),
        };
        self.rt.block_on(async {
            self.client.execute(&sql, &[&id]).await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn get_all_groups(&self) -> Result<Vec<GroupRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query("select id, coalesce(name,'') from grup order by id", &[])
                .await?;
            let out = rows
                .into_iter()
                .map(|r| GroupRow {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn get_regs_for_group(&self, group_id: i32) -> Result<Vec<RegRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, coalesce(name,''), coalesce(mb,0), coalesce(tip,0), bits \
                     from reg where grup = $1 order by mb asc nulls last, id asc",
                    &[&group_id],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| RegRow {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                    mb: r.get::<_, i32>(2),
                    tip: r.get::<_, i32>(3),
                    bits: r.try_get::<_, Option<i32>>(4).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn get_kpz_io_rows(&self, kpz_id: i32, group_id: i32, n_mb: i32) -> Result<Vec<KpzIoRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select r.id, coalesce(r.name,''), coalesce(r.mb,0), coalesce(r.tip,0), r.bits, \
                            r.val::double precision as reg_val, \
                            (select av.val_num \
                               from arx_val av \
                              where av.kpz_id = $1 and av.reg_id = r.id and av.val_num is not null \
                              order by av.ts_unix desc \
                              limit 1) as last_val \
                     from reg r \
                     where r.grup = $2 and coalesce(r.n_mb, 0) = $3 \
                     order by r.mb asc nulls last, r.id asc",
                    &[&kpz_id, &group_id, &n_mb],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| KpzIoRow {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                    mb: r.get::<_, i32>(2),
                    tip: r.get::<_, i32>(3),
                    bits: r.try_get::<_, Option<i32>>(4).ok().flatten(),
                    reg_val: r.try_get::<_, Option<f64>>(5).ok().flatten(),
                    last_val: r.try_get::<_, Option<f64>>(6).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn update_reg_val(&self, reg_id: i32, val: f64) -> Result<()> {
        let sval = val.to_string();
        self.rt.block_on(async {
            self.client
                .execute("update reg set val = $1 where id = $2", &[&sval, &reg_id])
                .await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn update_reg_val_checked(&self, reg_id: i32, val: f64) -> Result<u64> {
        let sval = val.to_string();
        self.rt.block_on(async {
            let n = self
                .client
                .execute("update reg set val = $1 where id = $2", &[&sval, &reg_id])
                .await?;
            Ok(n)
        })
    }

    /// Function: $name.
    #[allow(dead_code)]
    pub fn update_kpz_meta(
        &self,
        id: i32,
        start: i32,
        t_a: Option<i32>,
        t_script: Option<i32>,
    ) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute(
                    "update kpz \
                     set start = $1, \
                         t_a = $2, \
                         t_script = $3 \
                     where id = $4",
                    &[&start, &t_a, &t_script, &id],
                )
                .await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn update_kpz_full(
        &self,
        id: i32,
        name: &str,
        rtu: i32,
        obj: i32,
        modem: Option<i32>,
        max_pkt_len: i32,
        start: i32,
        t_a: Option<i32>,
        t_script: Option<i32>,
        en_post: bool,
    ) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute(
                    "update kpz \
                     set name = $1, \
                         rtu = $2, \
                         obj = $3, \
                         modem = $4, \
                         max_pkt_len = $5, \
                         start = $6, \
                         t_a = $7, \
                         t_script = $8, \
                         en_post = $9 \
                     where id = $10",
                    &[
                        &name,
                        &rtu,
                        &obj,
                        &modem,
                        &max_pkt_len,
                        &start,
                        &t_a,
                        &t_script,
                        &en_post,
                        &id,
                    ],
                )
                .await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn create_kpz_new(&self, obj_id: i32, id: Option<i32>) -> Result<i32> {
        self.rt.block_on(async {
            let row = if let Some(id) = id {
                self.client
                    .query_one(
                        "insert into kpz(id, name, rtu, obj, modem, grups, max_pkt_len, start, t_a, t_script, en_post)
                         values(
                            $1::int,
                            format('new_kpz_%s', $1::int),
                            $1::int,
                            $2::int,
                            50002,
                            decode(repeat('00', 64), 'hex'),
                            800,
                            0,
                            0,
                            0,
                            false
                         )
                         returning id",
                        &[&id, &obj_id],
                    )
                    .await?
            } else {
                self.client
                    .query_one(
                        "with next_id as (
                             select coalesce(max(id), 0) + 1 as id
                             from kpz
                         )
                         insert into kpz(id, name, rtu, obj, modem, grups, max_pkt_len, start, t_a, t_script, en_post)
                         select n.id,
                                format('new_kpz_%s', n.id),
                                n.id,
                                $1::int,
                                50002,
                                decode(repeat('00', 64), 'hex'),
                                800,
                                0,
                                0,
                                0,
                                false
                         from next_id n
                         returning id",
                        &[&obj_id],
                    )
                    .await?
            };
            Ok(row.get::<_, i32>(0))
        })
    }

    /// Function: $name.
    pub fn upsert_test_kpz_range(
        &self,
        id_start: i32,
        id_end: i32,
        obj_id: i32,
        modem_start: i32,
        max_pkt_len: i32,
    ) -> Result<u64> {
        self.rt.block_on(async {
            let n = self
                .client
                .execute(
                    "insert into kpz(id, name, rtu, obj, modem, grups, max_pkt_len, start, t_a, t_script)
                     select gs,
                            format('test_kpz_%s', gs),
                            301,
                            $3::int,
                            ($4::int + (gs - $1::int)),
                            decode(repeat('00', 64), 'hex'),
                            $5::int,
                            0,
                            0,
                            0
                     from generate_series($1::int, $2::int) as gs
                     on conflict (id) do update
                     set name = excluded.name,
                         rtu = excluded.rtu,
                         obj = excluded.obj,
                         modem = excluded.modem,
                         max_pkt_len = excluded.max_pkt_len",
                    &[&id_start, &id_end, &obj_id, &modem_start, &max_pkt_len],
                )
                .await?;
            Ok(n)
        })
    }

    /// Function: $name.
    pub fn set_kpz_start_range(&self, id_start: i32, id_end: i32, start: bool) -> Result<u64> {
        let start_i = if start { 1 } else { 0 };
        self.rt.block_on(async {
            let n = self
                .client
                .execute(
                    "update kpz set start = $3 where id between $1 and $2",
                    &[&id_start, &id_end, &start_i],
                )
                .await?;
            Ok(n)
        })
    }

    /// Function: $name.
    pub fn set_kpz_timing_range(
        &self,
        id_start: i32,
        id_end: i32,
        t_a: Option<i32>,
        t_script: Option<i32>,
    ) -> Result<u64> {
        self.rt.block_on(async {
            let n = self
                .client
                .execute(
                    "update kpz set t_a = $3, t_script = $4 where id between $1 and $2",
                    &[&id_start, &id_end, &t_a, &t_script],
                )
                .await?;
            Ok(n)
        })
    }

    /// Function: $name.
    pub fn update_obj(
        &self,
        id: i32,
        name: &str,
        ip: Option<i32>,
        port: Option<i32>,
        kanal: Option<i32>,
        speed: Option<i32>,
        stop: Option<i32>,
        parit: Option<i32>,
        bit: Option<i32>,
    ) -> Result<()> {
        let ip_text: Option<String> = ip.map(|v| v.to_string());
        self.rt.block_on(async {
            self.client
                .execute(
                    "update obj \
                     set name = $1, \
                         ip = $2, \
                         port = $3, \
                         kanal = $4, \
                         speed = $5, \
                         stop = $6, \
                         parit = $7, \
                         bit = $8 \
                     where id = $9",
                    &[&name, &ip_text, &port, &kanal, &speed, &stop, &parit, &bit, &id],
                )
                .await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn create_obj(
        &self,
        name: &str,
        ip: Option<i32>,
        port: Option<i32>,
        kanal: Option<i32>,
        speed: Option<i32>,
        stop: Option<i32>,
        parit: Option<i32>,
        bit: Option<i32>,
    ) -> Result<i32> {
        let ip_text: Option<String> = ip.map(|v| v.to_string());
        self.rt.block_on(async {
            let row = self
                .client
                .query_one(
                    "insert into obj (id, name, ip, port, kanal, speed, stop, parit, bit) \
                     values ((select coalesce(max(id), 0) + 1 from obj), $1, $2, $3, $4, $5, $6, $7, $8) \
                     returning id",
                    &[&name, &ip_text, &port, &kanal, &speed, &stop, &parit, &bit],
                )
                .await?;
            Ok(row.get::<_, i32>(0))
        })
    }

    /// Function: $name.
    pub fn get_all_reg_edit(&self) -> Result<Vec<RegEditRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, coalesce(name,''), coalesce(mb,0), n_mb, coalesce(tip,0), bits, grup, \
                     case when a_en::text in ('1','t','true','T','TRUE') then 1 else 0 end as a_en_i, \
                     coalesce(a_no_write,0) \
                     from reg order by id",
                    &[],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| RegEditRow {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                    mb: r.get::<_, i32>(2),
                    n_mb: r.try_get::<_, Option<i32>>(3).ok().flatten(),
                    tip: r.get::<_, i32>(4),
                    bits: r.try_get::<_, Option<i32>>(5).ok().flatten(),
                    grup: r.try_get::<_, Option<i32>>(6).ok().flatten(),
                    a_en: r.get::<_, i32>(7) != 0,
                    a_no_write: r.get::<_, i32>(8),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn update_reg_edit(
        &self,
        id: i32,
        name: &str,
        mb: i32,
        n_mb: Option<i32>,
        tip: i32,
        bits: Option<i32>,
        grup: i32,
        a_en: bool,
        a_no_write: i32,
    ) -> Result<()> {
        let a_no_write_i16 = a_no_write.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        self.rt.block_on(async {
            self.client
                .execute(
                    "insert into reg(id, name, mb, n_mb, tip, bits, grup, a_en, a_no_write) \
                     values($9, $1, $2, $3, $4, $5, $6, $7, $8) \
                     on conflict (id) do update \
                     set name = excluded.name, \
                         mb = excluded.mb, \
                         n_mb = excluded.n_mb, \
                         tip = excluded.tip, \
                         bits = excluded.bits, \
                         grup = excluded.grup, \
                         a_en = excluded.a_en, \
                         a_no_write = excluded.a_no_write",
                    &[&name, &mb, &n_mb, &tip, &bits, &grup, &a_en, &a_no_write_i16, &id],
                )
                .await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn update_kpz_grups(&self, id: i32, grups: &[u8]) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute("update kpz set grups = $1 where id = $2", &[&grups, &id])
                .await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn get_poll_log(&self, kpz_id: Option<i32>, limit: i64) -> Result<Vec<PollLogRow>> {
        self.rt.block_on(async {
            let rows = if let Some(k) = kpz_id {
                self.client
                    .query(
                        "select to_char(l.ts,'YYYY-MM-DD HH24:MI:SS.MS') as ts, l.kpz_id, \
                         coalesce(l.kind,''), coalesce(l.msg,'') \
                         from poll_log l where l.kpz_id = $1 \
                         order by l.ts desc, l.id desc limit $2",
                        &[&k, &limit],
                    )
                    .await?
            } else {
                self.client
                    .query(
                        "select to_char(l.ts,'YYYY-MM-DD HH24:MI:SS.MS') as ts, l.kpz_id, \
                         coalesce(l.kind,''), coalesce(l.msg,'') \
                         from poll_log l \
                         order by l.ts desc, l.id desc limit $1",
                        &[&limit],
                    )
                    .await?
            };

            let out = rows
                .into_iter()
                .map(|r| PollLogRow {
                    ts: r.get::<_, String>(0),
                    kpz_id: r.try_get::<_, Option<i32>>(1).ok().flatten(),
                    kind: r.get::<_, String>(2),
                    msg: r.get::<_, String>(3),
                })
                .collect();
            Ok(out)
        })
    }

    /// Загружает последние записи `elam` с необязательной фильтрацией по `kpz_id` и временному интервалу.
    ///
    /// # Parameters
    /// - `kpz_id`: `Some(id)` — только по указанному КПЗ, `None` — без фильтра.
    /// - `limit`: максимальное число возвращаемых строк.
    /// - `ts_from_unix`: нижняя граница `e.ts` (Unix sec, включительно), `None` — без нижней границы.
    /// - `ts_to_unix`: верхняя граница `e.ts` (Unix sec, включительно), `None` — без верхней границы.
    ///
    /// # Returns
    /// - `Ok(Vec<ElamRow>)`: список строк `elam`, отсортированный по `ts desc, id desc`.
    /// - `Err(...)`: ошибка SQL/декодирования.
    pub fn get_last_elam(
        &self,
        kpz_id: Option<i32>,
        limit: i64,
        ts_from_unix: Option<i64>,
        ts_to_unix: Option<i64>,
    ) -> Result<Vec<ElamRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select e.id, to_char(e.ts,'YYYY-MM-DD HH24:MI:SS.MS') as ts, \
                     e.kpz_id, e.group_id, coalesce(e.status,''), e.duration_ms, \
                     e.func, e.addr_human, e.count_words, e.req, e.resp \
                     from elam e \
                     where ($1::int4 is null or e.kpz_id = $1) \
                       and ($2::int8 is null or e.ts >= to_timestamp($2)) \
                       and ($3::int8 is null or e.ts <= to_timestamp($3)) \
                     order by e.ts desc, e.id desc limit $4",
                    &[&kpz_id, &ts_from_unix, &ts_to_unix, &limit],
                )
                .await?;

            let out = rows
                .into_iter()
                .map(|r| ElamRow {
                    id: r.get::<_, i64>(0),
                    ts: r.get::<_, String>(1),
                    kpz_id: r.get::<_, i32>(2),
                    group_id: r.try_get::<_, Option<i32>>(3).ok().flatten(),
                    status: r.get::<_, String>(4),
                    duration_ms: r.try_get::<_, Option<i32>>(5).ok().flatten(),
                    func: r.try_get::<_, Option<i32>>(6).ok().flatten(),
                    addr_human: r.try_get::<_, Option<i32>>(7).ok().flatten(),
                    count_words: r.try_get::<_, Option<i32>>(8).ok().flatten(),
                    req: r.try_get::<_, Vec<u8>>(9).unwrap_or_default(),
                    resp: r.try_get::<_, Option<Vec<u8>>>(10).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn get_g_script(&self, grup: i32) -> Result<Option<GScriptRow>> {
        self.rt.block_on(async {
            let row = self
                .client
                .query_opt(
                    "select grup, elam, max, max_k, pre_src, post_src, en, ver \
                     from g_script where grup = $1 limit 1",
                    &[&grup],
                )
                .await?;
            let Some(r) = row else {
                return Ok(None);
            };
            let elam = r
                .try_get::<_, Option<i16>>(1)
                .ok()
                .flatten()
                .map(|v| v as i32)
                .unwrap_or(0);
            let max_words = r.try_get::<_, Option<i32>>(2).ok().flatten().unwrap_or(800);
            let max_k = r.try_get::<_, Option<i32>>(3).ok().flatten().unwrap_or(2);
            let pre_src = r.try_get::<_, Option<String>>(4).ok().flatten().unwrap_or_default();
            let post_src = r.try_get::<_, Option<String>>(5).ok().flatten().unwrap_or_default();
            let en = r.try_get::<_, Option<bool>>(6).ok().flatten().unwrap_or(true);
            let ver = r.try_get::<_, Option<i32>>(7).ok().flatten().unwrap_or(1);
            Ok(Some(GScriptRow {
                grup: r.get::<_, i32>(0),
                elam,
                max_words,
                max_k,
                pre_src,
                post_src,
                en,
                ver,
            }))
        })
    }

    /// Function: $name.
    pub fn list_g_script_groups(&self) -> Result<Vec<i32>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query("select grup from g_script order by grup", &[])
                .await?;
            Ok(rows.into_iter().map(|r| r.get::<_, i32>(0)).collect())
        })
    }

    /// Function: $name.
    pub fn list_g_script_templates(&self) -> Result<Vec<GScriptTemplateRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, coalesce(name,''), coalesce(pre_src,''), coalesce(post_src,''), \
                            coalesce(elam,0), coalesce(max_words,800), coalesce(max_k,2), \
                            coalesce(en,true), coalesce(ver,1) \
                     from ui.gscript_template order by name, id",
                    &[],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| GScriptTemplateRow {
                    id: r.get::<_, i64>(0),
                    name: r.get::<_, String>(1),
                    pre_src: r.get::<_, String>(2),
                    post_src: r.get::<_, String>(3),
                    elam: r.get::<_, i16>(4) as i32,
                    max_words: r.get::<_, i32>(5),
                    max_k: r.get::<_, i32>(6),
                    en: r.get::<_, bool>(7),
                    ver: r.get::<_, i32>(8),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn upsert_g_script_template(&self, row: &GScriptTemplateRow) -> Result<i64> {
        let elam_i16 = row.elam.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        self.rt.block_on(async {
            let id = if row.id > 0 {
                let r = self
                    .client
                    .query_opt(
                        "update ui.gscript_template \
                         set name=$2, pre_src=$3, post_src=$4, elam=$5, max_words=$6, max_k=$7, \
                             en=$8, ver=$9, updated_at=now() \
                         where id=$1 \
                         returning id",
                        &[
                            &row.id,
                            &row.name,
                            &row.pre_src,
                            &row.post_src,
                            &elam_i16,
                            &row.max_words,
                            &row.max_k,
                            &row.en,
                            &row.ver,
                        ],
                    )
                    .await?;
                if let Some(found) = r {
                    found.get::<_, i64>(0)
                } else {
                    let r = self
                        .client
                        .query_one(
                            "insert into ui.gscript_template(name, pre_src, post_src, elam, max_words, max_k, en, ver) \
                             values($1,$2,$3,$4,$5,$6,$7,$8) \
                             on conflict (name) do update set \
                               pre_src=excluded.pre_src, post_src=excluded.post_src, \
                               elam=excluded.elam, max_words=excluded.max_words, max_k=excluded.max_k, \
                               en=excluded.en, ver=excluded.ver, updated_at=now() \
                             returning id",
                            &[
                                &row.name,
                                &row.pre_src,
                                &row.post_src,
                                &elam_i16,
                                &row.max_words,
                                &row.max_k,
                                &row.en,
                                &row.ver,
                            ],
                        )
                        .await?;
                    r.get::<_, i64>(0)
                }
            } else {
                let r = self
                    .client
                    .query_one(
                        "insert into ui.gscript_template(name, pre_src, post_src, elam, max_words, max_k, en, ver) \
                         values($1,$2,$3,$4,$5,$6,$7,$8) \
                         on conflict (name) do update set \
                           pre_src=excluded.pre_src, post_src=excluded.post_src, \
                           elam=excluded.elam, max_words=excluded.max_words, max_k=excluded.max_k, \
                           en=excluded.en, ver=excluded.ver, updated_at=now() \
                         returning id",
                        &[
                            &row.name,
                            &row.pre_src,
                            &row.post_src,
                            &elam_i16,
                            &row.max_words,
                            &row.max_k,
                            &row.en,
                            &row.ver,
                        ],
                    )
                    .await?;
                r.get::<_, i64>(0)
            };
            Ok(id)
        })
    }

    /// Function: $name.
    pub fn delete_g_script_template(&self, id: i64) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute("delete from ui.gscript_template where id = $1", &[&id])
                .await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn get_group_template_id(&self, group_id: i32) -> Result<Option<i64>> {
        self.rt.block_on(async {
            let row = self
                .client
                .query_opt(
                    "select template_id from ui.gscript_group_template where group_id=$1",
                    &[&group_id],
                )
                .await?;
            Ok(row.map(|r| r.get::<_, i64>(0)))
        })
    }

    /// Function: $name.
    pub fn set_group_template(&self, group_id: i32, template_id: Option<i64>) -> Result<()> {
        self.rt.block_on(async {
            if let Some(tid) = template_id {
                self.client
                    .execute(
                        "insert into ui.gscript_group_template(group_id, template_id) values($1,$2) \
                         on conflict (group_id) do update set template_id=excluded.template_id, updated_at=now()",
                        &[&group_id, &tid],
                    )
                    .await?;
            } else {
                self.client
                    .execute(
                        "delete from ui.gscript_group_template where group_id=$1",
                        &[&group_id],
                    )
                    .await?;
            }
            Ok(())
        })
    }

    /// Function: $name.
    pub fn get_effective_g_script(&self, grup: i32) -> Result<Option<GScriptRow>> {
        if let Some(row) = self.get_g_script(grup)? {
            return Ok(Some(row));
        }
        self.rt.block_on(async {
            let row = self
                .client
                .query_opt(
                    "select gt.id, gt.elam, gt.max_words, gt.max_k, gt.pre_src, gt.post_src, gt.en, gt.ver \
                     from ui.gscript_group_template ggt \
                     join ui.gscript_template gt on gt.id = ggt.template_id \
                     where ggt.group_id = $1 \
                     limit 1",
                    &[&grup],
                )
                .await?;
            let Some(r) = row else {
                return Ok(None);
            };
            Ok(Some(GScriptRow {
                grup,
                elam: r.get::<_, i16>(1) as i32,
                max_words: r.get::<_, i32>(2),
                max_k: r.get::<_, i32>(3),
                pre_src: r.get::<_, String>(4),
                post_src: r.get::<_, String>(5),
                en: r.get::<_, bool>(6),
                ver: r.get::<_, i32>(7),
            }))
        })
    }

    /// Function: $name.
    pub fn get_scheduler_runtime_cfg(&self) -> Result<SchedulerRuntimeCfgRow> {
        self.rt.block_on(async {
            let row = self
                .client
                .query_opt(
                    "select id, \
                            coalesce(no_response_failures, 3)::int4, \
                            coalesce(no_response_backoff_sec, 600)::int8, \
                            coalesce(metrics_p95_warn_ms, 1000)::int8, \
                            coalesce(metrics_p95_crit_ms, 3000)::int8, \
                            coalesce(modbus_a_timeout_ms, 1800)::int8, \
                            coalesce(modbus_script_timeout_ms, 2600)::int8 \
                     from public.scheduler_runtime_cfg \
                     order by id \
                     limit 1",
                    &[],
                )
                .await?;
            let Some(r) = row else {
                return Ok(SchedulerRuntimeCfgRow {
                    id: 1,
                    no_response_failures: 3,
                    no_response_backoff_sec: 600,
                    metrics_p95_warn_ms: 1000,
                    metrics_p95_crit_ms: 3000,
                    modbus_a_timeout_ms: 1800,
                    modbus_script_timeout_ms: 2600,
                });
            };
            Ok(SchedulerRuntimeCfgRow {
                id: r.get::<_, i64>(0),
                no_response_failures: r.get::<_, i32>(1),
                no_response_backoff_sec: r.get::<_, i64>(2),
                metrics_p95_warn_ms: r.get::<_, i64>(3),
                metrics_p95_crit_ms: r.get::<_, i64>(4),
                modbus_a_timeout_ms: r.get::<_, i64>(5),
                modbus_script_timeout_ms: r.get::<_, i64>(6),
            })
        })
    }

    /// Function: $name.
    pub fn upsert_scheduler_runtime_cfg(
        &self,
        no_response_failures: i32,
        no_response_backoff_sec: i64,
        metrics_p95_warn_ms: i64,
        metrics_p95_crit_ms: i64,
        modbus_a_timeout_ms: i64,
        modbus_script_timeout_ms: i64,
    ) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute(
                    "insert into public.scheduler_runtime_cfg(id, no_response_failures, no_response_backoff_sec, metrics_p95_warn_ms, metrics_p95_crit_ms, modbus_a_timeout_ms, modbus_script_timeout_ms) \
                     values (1, $1, $2, $3, $4, $5, $6) \
                     on conflict (id) do update set \
                       no_response_failures = excluded.no_response_failures, \
                       no_response_backoff_sec = excluded.no_response_backoff_sec, \
                       metrics_p95_warn_ms = excluded.metrics_p95_warn_ms, \
                       metrics_p95_crit_ms = excluded.metrics_p95_crit_ms, \
                       modbus_a_timeout_ms = excluded.modbus_a_timeout_ms, \
                       modbus_script_timeout_ms = excluded.modbus_script_timeout_ms, \
                       updated_at = now()",
                    &[
                        &no_response_failures,
                        &no_response_backoff_sec,
                        &metrics_p95_warn_ms,
                        &metrics_p95_crit_ms,
                        &modbus_a_timeout_ms,
                        &modbus_script_timeout_ms,
                    ],
                )
                .await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn get_last_arx_vals(&self, kpz_id: i32) -> Result<std::collections::HashMap<i32, f64>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select distinct on (reg_id) reg_id, val_num \
                     from arx_val \
                     where kpz_id = $1 and val_num is not null \
                     order by reg_id, ts_unix desc",
                    &[&kpz_id],
                )
                .await?;
            let mut out = std::collections::HashMap::new();
            for r in rows {
                let reg_id = r.get::<_, i32>(0);
                let val = r.get::<_, f64>(1);
                out.insert(reg_id, val);
            }
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn get_arx_series(
        &self,
        kpz_id: i32,
        reg_ids: &[i32],
        limit: i64,
        window_sec: i64,
    ) -> Result<Vec<ArxSeriesRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select reg_id, ts_unix, val_num \
                     from arx_val \
                     where kpz_id = $1 and reg_id = any($2) and val_num is not null \
                       and ts_unix >= (extract(epoch from now())::bigint - $4) \
                       and ts_unix <= (extract(epoch from now())::bigint + 86400) \
                     order by ts_unix desc \
                     limit $3",
                    &[&kpz_id, &reg_ids, &limit, &window_sec],
                )
                .await?;

            let mut by_reg: std::collections::BTreeMap<i32, Vec<ArxPointRow>> =
                std::collections::BTreeMap::new();
            for r in rows {
                let reg_id = r.get::<_, i32>(0);
                let ts_unix = r.get::<_, i64>(1);
                let val_num = r.get::<_, f64>(2);
                by_reg
                    .entry(reg_id)
                    .or_default()
                    .push(ArxPointRow { ts_unix, val_num });
            }

            let mut out = Vec::new();
            for (reg_id, mut points) in by_reg {
                points.reverse();
                out.push(ArxSeriesRow { reg_id, points });
            }
            Ok(out)
        })
    }

    /// Returns rows from `public.arx_state` ordered by `updated_at desc`.
    ///
    /// If `kpz_id` is provided, applies per-KPZ filter.
    pub fn get_arx_state_rows(&self, kpz_id: Option<i32>, limit: i64) -> Result<Vec<ArxStateRow>> {
        self.rt.block_on(async {
            let rows = if let Some(k) = kpz_id {
                self.client
                    .query(
                        "select kpz_id, arx_id, last_ind, to_char(updated_at,'YYYY-MM-DD HH24:MI:SS.MS') as updated_at \
                         from arx_state \
                         where kpz_id = $1 \
                         order by updated_at desc, kpz_id, arx_id \
                         limit $2",
                        &[&k, &limit],
                    )
                    .await?
            } else {
                self.client
                    .query(
                        "select kpz_id, arx_id, last_ind, to_char(updated_at,'YYYY-MM-DD HH24:MI:SS.MS') as updated_at \
                         from arx_state \
                         order by updated_at desc, kpz_id, arx_id \
                         limit $1",
                        &[&limit],
                    )
                    .await?
            };
            let out = rows
                .into_iter()
                .map(|r| ArxStateRow {
                    kpz_id: r.get::<_, i32>(0),
                    arx_id: r.get::<_, i32>(1),
                    last_ind: r.get::<_, i32>(2),
                    updated_at: r.get::<_, String>(3),
                })
                .collect();
            Ok(out)
        })
    }

    /// Returns latest rows from `public.arx_val` for quick runtime inspection.
    ///
    /// If `kpz_id` is provided, applies per-KPZ filter.
    pub fn get_last_arx_val_rows(&self, kpz_id: Option<i32>, limit: i64) -> Result<Vec<ArxValViewRow>> {
        self.rt.block_on(async {
            let rows = if let Some(k) = kpz_id {
                self.client
                    .query(
                        "select id, kpz_id, reg_id, tip, val_num, ts_unix, \
                                to_char(created_at,'YYYY-MM-DD HH24:MI:SS.MS') as created_at \
                         from arx_val \
                         where kpz_id = $1 \
                         order by created_at desc, id desc \
                         limit $2",
                        &[&k, &limit],
                    )
                    .await?
            } else {
                self.client
                    .query(
                        "select id, kpz_id, reg_id, tip, val_num, ts_unix, \
                                to_char(created_at,'YYYY-MM-DD HH24:MI:SS.MS') as created_at \
                         from arx_val \
                         order by created_at desc, id desc \
                         limit $1",
                        &[&limit],
                    )
                    .await?
            };

            let out = rows
                .into_iter()
                .map(|r| ArxValViewRow {
                    id: r.get::<_, i64>(0),
                    kpz_id: r.get::<_, i32>(1),
                    reg_id: r.get::<_, i32>(2),
                    tip: r
                        .try_get::<_, i16>(3)
                        .ok()
                        .or_else(|| r.try_get::<_, i32>(3).ok().map(|v| v as i16))
                        .unwrap_or(0),
                    val_num: r.try_get::<_, Option<f64>>(4).ok().flatten(),
                    ts_unix: r.get::<_, i64>(5),
                    created_at: r.get::<_, String>(6),
                })
                .collect();
            Ok(out)
        })
    }

    /// Upserts a single `(kpz_id, arx_id)` entry in `public.arx_state`.
    pub fn upsert_arx_state(&self, kpz_id: i32, arx_id: i32, last_ind: i32) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute(
                    "insert into arx_state(kpz_id, arx_id, last_ind, updated_at) \
                     values($1, $2, $3, now()) \
                     on conflict (kpz_id, arx_id) do update set \
                       last_ind = excluded.last_ind, \
                       updated_at = now()",
                    &[&kpz_id, &arx_id, &last_ind],
                )
                .await?;
            Ok(())
        })
    }

    /// Deletes rows from `arx_val` globally or by `kpz_id`.
    pub fn clear_arx_val(&self, kpz_id: Option<i32>) -> Result<u64> {
        self.rt.block_on(async {
            let n = if let Some(k) = kpz_id {
                self.client
                    .execute("delete from arx_val where kpz_id = $1", &[&k])
                    .await?
            } else {
                self.client.execute("delete from arx_val", &[]).await?
            };
            Ok(n)
        })
    }

    /// Deletes rows from `elam` globally or by `kpz_id`.
    pub fn clear_elam(&self, kpz_id: Option<i32>) -> Result<u64> {
        self.rt.block_on(async {
            let n = if let Some(k) = kpz_id {
                self.client
                    .execute("delete from elam where kpz_id = $1", &[&k])
                    .await?
            } else {
                self.client.execute("delete from elam", &[]).await?
            };
            Ok(n)
        })
    }

    /// Deletes rows from `poll_log`.
    ///
    /// For filtered mode also removes `kpz_id is null` service rows.
    pub fn clear_poll_log(&self, kpz_id: Option<i32>) -> Result<u64> {
        self.rt.block_on(async {
            let n = if let Some(k) = kpz_id {
                self.client
                    .execute("delete from poll_log where kpz_id = $1 or kpz_id is null", &[&k])
                    .await?
            } else {
                self.client.execute("delete from poll_log", &[]).await?
            };
            Ok(n)
        })
    }

    /// Function: $name.
    pub fn upsert_g_script(&self, row: &GScriptRow) -> Result<()> {
        let elam_i16 = row.elam.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        self.rt.block_on(async {
            self.client
                .execute(
                    "insert into g_script(grup, elam, max, max_k, pre_src, post_src, en, ver) \
                     values($1,$2,$3,$4,$5,$6,$7,$8) \
                     on conflict (grup) do update set \
                     elam=excluded.elam, max=excluded.max, max_k=excluded.max_k, \
                     pre_src=excluded.pre_src, post_src=excluded.post_src, \
                     en=excluded.en, ver=excluded.ver, updated_at=now()",
                    &[
                        &row.grup,
                        &elam_i16,
                        &row.max_words,
                        &row.max_k,
                        &row.pre_src,
                        &row.post_src,
                        &row.en,
                        &row.ver,
                    ],
                )
                .await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn get_alarm_rules(&self, kpz_id: Option<i32>) -> Result<Vec<AlarmRuleRow>> {
        self.rt.block_on(async {
            let rows = if let Some(k) = kpz_id {
                self.client
                    .query(
                        "select id, kpz_id, reg_id, enabled, cmp, set_lo, set_hi, set_lo_1, set_hi_1, \
                                coalesce(hysteresis,0), coalesce(on_delay_sec,0), coalesce(off_delay_sec,0), \
                                coalesce(severity,1), code, message, chat_id, \
                                coalesce(tg_on_on, true), coalesce(tg_on_off, false), \
                                coalesce(tg_thr_main, true), coalesce(tg_thr_lvl1, true) \
                         from alarm_rule where kpz_id = $1 order by kpz_id, reg_id, id",
                        &[&k],
                    )
                    .await?
            } else {
                self.client
                    .query(
                        "select id, kpz_id, reg_id, enabled, cmp, set_lo, set_hi, set_lo_1, set_hi_1, \
                                coalesce(hysteresis,0), coalesce(on_delay_sec,0), coalesce(off_delay_sec,0), \
                                coalesce(severity,1), code, message, chat_id, \
                                coalesce(tg_on_on, true), coalesce(tg_on_off, false), \
                                coalesce(tg_thr_main, true), coalesce(tg_thr_lvl1, true) \
                         from alarm_rule order by kpz_id, reg_id, id",
                        &[],
                    )
                    .await?
            };
            let out = rows
                .into_iter()
                .map(|r| AlarmRuleRow {
                    id: r.get::<_, i64>(0),
                    kpz_id: r.get::<_, i32>(1),
                    reg_id: r.get::<_, i32>(2),
                    enabled: r
                        .try_get::<_, bool>(3)
                        .ok()
                        .or_else(|| r.try_get::<_, i16>(3).ok().map(|v| v != 0))
                        .or_else(|| r.try_get::<_, i32>(3).ok().map(|v| v != 0))
                        .unwrap_or(true),
                    cmp: r.get::<_, String>(4),
                    set_lo: r.try_get::<_, Option<f64>>(5).ok().flatten(),
                    set_hi: r.try_get::<_, Option<f64>>(6).ok().flatten(),
                    set_lo_1: r.try_get::<_, Option<f64>>(7).ok().flatten(),
                    set_hi_1: r.try_get::<_, Option<f64>>(8).ok().flatten(),
                    hysteresis: r.get::<_, f64>(9),
                    on_delay_sec: r.get::<_, i32>(10),
                    off_delay_sec: r.get::<_, i32>(11),
                    severity: r
                        .try_get::<_, i16>(12)
                        .ok()
                        .or_else(|| r.try_get::<_, i32>(12).ok().map(|v| v as i16))
                        .unwrap_or(1),
                    code: r.try_get::<_, Option<String>>(13).ok().flatten(),
                    message: r.try_get::<_, Option<String>>(14).ok().flatten(),
                    chat_id: r.try_get::<_, Option<String>>(15).ok().flatten(),
                    tg_on_on: r.try_get::<_, bool>(16).ok().unwrap_or(true),
                    tg_on_off: r.try_get::<_, bool>(17).ok().unwrap_or(false),
                    tg_thr_main: r.try_get::<_, bool>(18).ok().unwrap_or(true),
                    tg_thr_lvl1: r.try_get::<_, bool>(19).ok().unwrap_or(true),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn upsert_alarm_rule(&self, row: &AlarmRuleRow) -> Result<i64> {
        self.rt.block_on(async {
            let r = self
                .client
                .query_one(
                    "insert into alarm_rule(id, kpz_id, reg_id, enabled, cmp, set_lo, set_hi, set_lo_1, set_hi_1, hysteresis, on_delay_sec, off_delay_sec, severity, code, message, chat_id, tg_on_on, tg_on_off, tg_thr_main, tg_thr_lvl1) \
                     values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20) \
                     on conflict (id) do update set \
                       kpz_id=excluded.kpz_id, reg_id=excluded.reg_id, enabled=excluded.enabled, cmp=excluded.cmp, \
                       set_lo=excluded.set_lo, set_hi=excluded.set_hi, set_lo_1=excluded.set_lo_1, set_hi_1=excluded.set_hi_1, hysteresis=excluded.hysteresis, \
                       on_delay_sec=excluded.on_delay_sec, off_delay_sec=excluded.off_delay_sec, severity=excluded.severity, \
                       code=excluded.code, message=excluded.message, chat_id=excluded.chat_id, \
                       tg_on_on=excluded.tg_on_on, tg_on_off=excluded.tg_on_off, \
                       tg_thr_main=excluded.tg_thr_main, tg_thr_lvl1=excluded.tg_thr_lvl1, updated_at=now() \
                     returning id",
                    &[
                        &row.id,
                        &row.kpz_id,
                        &row.reg_id,
                        &row.enabled,
                        &row.cmp,
                        &row.set_lo,
                        &row.set_hi,
                        &row.set_lo_1,
                        &row.set_hi_1,
                        &row.hysteresis,
                        &row.on_delay_sec,
                        &row.off_delay_sec,
                        &row.severity,
                        &row.code,
                        &row.message,
                        &row.chat_id,
                        &row.tg_on_on,
                        &row.tg_on_off,
                        &row.tg_thr_main,
                        &row.tg_thr_lvl1,
                    ],
                )
                .await?;
            Ok(r.get::<_, i64>(0))
        })
    }

    /// Function: $name.
    pub fn insert_alarm_rule(&self, row: &AlarmRuleRow) -> Result<i64> {
        self.rt.block_on(async {
            let r = self
                .client
                .query_one(
                    "insert into alarm_rule(kpz_id, reg_id, enabled, cmp, set_lo, set_hi, set_lo_1, set_hi_1, hysteresis, on_delay_sec, off_delay_sec, severity, code, message, chat_id, tg_on_on, tg_on_off, tg_thr_main, tg_thr_lvl1) \
                     values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19) returning id",
                    &[
                        &row.kpz_id,
                        &row.reg_id,
                        &row.enabled,
                        &row.cmp,
                        &row.set_lo,
                        &row.set_hi,
                        &row.set_lo_1,
                        &row.set_hi_1,
                        &row.hysteresis,
                        &row.on_delay_sec,
                        &row.off_delay_sec,
                        &row.severity,
                        &row.code,
                        &row.message,
                        &row.chat_id,
                        &row.tg_on_on,
                        &row.tg_on_off,
                        &row.tg_thr_main,
                        &row.tg_thr_lvl1,
                    ],
                )
                .await?;
            Ok(r.get::<_, i64>(0))
        })
    }

    /// Function: $name.
    pub fn delete_alarm_rule(&self, id: i64) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute("delete from alarm_rule where id = $1", &[&id])
                .await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn get_alarm_state(&self, kpz_id: Option<i32>, limit: i64) -> Result<Vec<AlarmStateRow>> {
        self.rt.block_on(async {
            let rows = if let Some(k) = kpz_id {
                self.client
                    .query(
                        "select s.rule_id, r.kpz_id, r.reg_id, s.active, \
                                to_char(s.active_since,'YYYY-MM-DD HH24:MI:SS.MS') as active_since, \
                                s.last_value, to_char(s.updated_at,'YYYY-MM-DD HH24:MI:SS.MS') as updated_at \
                         from alarm_state s \
                         join alarm_rule r on r.id = s.rule_id \
                         where r.kpz_id = $1 \
                         order by s.updated_at desc \
                         limit $2",
                        &[&k, &limit],
                    )
                    .await?
            } else {
                self.client
                    .query(
                        "select s.rule_id, r.kpz_id, r.reg_id, s.active, \
                                to_char(s.active_since,'YYYY-MM-DD HH24:MI:SS.MS') as active_since, \
                                s.last_value, to_char(s.updated_at,'YYYY-MM-DD HH24:MI:SS.MS') as updated_at \
                         from alarm_state s \
                         join alarm_rule r on r.id = s.rule_id \
                         order by s.updated_at desc \
                         limit $1",
                        &[&limit],
                    )
                    .await?
            };
            let out = rows
                .into_iter()
                .map(|r| AlarmStateRow {
                    rule_id: r.get::<_, i64>(0),
                    kpz_id: r.get::<_, i32>(1),
                    reg_id: r.get::<_, i32>(2),
                    active: r
                        .try_get::<_, bool>(3)
                        .ok()
                        .or_else(|| r.try_get::<_, i16>(3).ok().map(|v| v != 0))
                        .or_else(|| r.try_get::<_, i32>(3).ok().map(|v| v != 0))
                        .unwrap_or(false),
                    active_since: r.try_get::<_, Option<String>>(4).ok().flatten(),
                    last_value: r.try_get::<_, Option<f64>>(5).ok().flatten(),
                    updated_at: r.get::<_, String>(6),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn get_alarm_events(&self, kpz_id: Option<i32>, limit: i64) -> Result<Vec<AlarmEventRow>> {
        self.rt.block_on(async {
            let rows = if let Some(k) = kpz_id {
                self.client
                    .query(
                        "select id, to_char(ts,'YYYY-MM-DD HH24:MI:SS.MS') as ts, kpz_id, reg_id, rule_id, event, \
                                value, set_lo, set_hi, severity, code, message \
                         from alarm_event where kpz_id = $1 \
                         order by ts desc, id desc limit $2",
                        &[&k, &limit],
                    )
                    .await?
            } else {
                self.client
                    .query(
                        "select id, to_char(ts,'YYYY-MM-DD HH24:MI:SS.MS') as ts, kpz_id, reg_id, rule_id, event, \
                                value, set_lo, set_hi, severity, code, message \
                         from alarm_event \
                         order by ts desc, id desc limit $1",
                        &[&limit],
                    )
                    .await?
            };
            let out = rows
                .into_iter()
                .map(|r| AlarmEventRow {
                    id: r.get::<_, i64>(0),
                    ts: r.get::<_, String>(1),
                    kpz_id: r.get::<_, i32>(2),
                    reg_id: r.get::<_, i32>(3),
                    rule_id: r.get::<_, i64>(4),
                    event: r.get::<_, String>(5),
                    value: r.try_get::<_, Option<f64>>(6).ok().flatten(),
                    set_lo: r.try_get::<_, Option<f64>>(7).ok().flatten(),
                    set_hi: r.try_get::<_, Option<f64>>(8).ok().flatten(),
                    severity: r
                        .try_get::<_, i16>(9)
                        .ok()
                        .or_else(|| r.try_get::<_, i32>(9).ok().map(|v| v as i16))
                        .unwrap_or(1),
                    code: r.try_get::<_, Option<String>>(10).ok().flatten(),
                    message: r.try_get::<_, Option<String>>(11).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn get_ui_kpz_windows(&self, kpz_id: i32) -> Result<Vec<UiKpzWindowRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, kpz_id, code, title, description, is_active \
                     from ui.kpz_window \
                     where kpz_id = $1 \
                     order by code",
                    &[&kpz_id],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| UiKpzWindowRow {
                    id: r.get::<_, i64>(0),
                    kpz_id: r.get::<_, i32>(1),
                    code: r.get::<_, String>(2),
                    title: r.get::<_, String>(3),
                    description: r.try_get::<_, Option<String>>(4).ok().flatten(),
                    is_active: r.get::<_, bool>(5),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn upsert_ui_kpz_window(
        &self,
        kpz_id: i32,
        code: &str,
        title: &str,
        description: Option<&str>,
        is_active: bool,
    ) -> Result<i64> {
        self.rt.block_on(async {
            let row = self
                .client
                .query_one(
                    "insert into ui.kpz_window(kpz_id, code, title, description, is_active) \
                     values($1, $2, $3, $4, $5) \
                     on conflict (kpz_id, code) do update set \
                       title = excluded.title, \
                       description = excluded.description, \
                       is_active = excluded.is_active, \
                       updated_at = now() \
                     returning id",
                    &[&kpz_id, &code, &title, &description, &is_active],
                )
                .await?;
            Ok(row.get::<_, i64>(0))
        })
    }

    /// Function: $name.
    pub fn get_ui_window_groups(&self, window_id: i64) -> Result<Vec<UiWindowGroupRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select group_id, pos \
                     from ui.kpz_window_group \
                     where window_id = $1 \
                     order by pos, group_id",
                    &[&window_id],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| UiWindowGroupRow {
                    group_id: r.get::<_, i32>(0),
                    pos: r.get::<_, i32>(1),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn save_ui_window_groups(&self, window_id: i64, groups: &[UiWindowGroupRow]) -> Result<()> {
        self.rt.block_on(async {
            self.client.execute("begin", &[]).await?;
            self.client
                .execute(
                "delete from ui.kpz_window_group where window_id = $1",
                &[&window_id],
            )
            .await?;

            for g in groups {
                if let Err(e) = self
                    .client
                    .execute(
                    "insert into ui.kpz_window_group(window_id, group_id, pos) values($1,$2,$3)",
                    &[&window_id, &g.group_id, &g.pos],
                )
                .await
                {
                    let _ = self.client.execute("rollback", &[]).await;
                    return Err(e.into());
                }
            }
            self.client.execute("commit", &[]).await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn get_ui_window_bindings(&self, window_id: i64) -> Result<Vec<UiWindowBindingRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select b.reg_id, b.pos, coalesce(b.x,20), coalesce(b.y,20), coalesce(b.w,120), coalesce(b.h,34), \
                            b.visible, b.writable, b.label_override, b.unit, b.fmt, \
                            coalesce(r.name,''), coalesce(r.mb,0), coalesce(r.tip,0), coalesce(r.n_mb,0), r.bits \
                     from ui.kpz_window_reg_binding b \
                     join public.reg r on r.id = b.reg_id \
                     where b.window_id = $1 \
                     order by b.pos, b.reg_id",
                    &[&window_id],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| UiWindowBindingRow {
                    reg_id: r.get::<_, i32>(0),
                    pos: r.get::<_, i32>(1),
                    x: r.get::<_, i32>(2),
                    y: r.get::<_, i32>(3),
                    w: r.get::<_, i32>(4),
                    h: r.get::<_, i32>(5),
                    visible: r.get::<_, bool>(6),
                    writable: r.get::<_, bool>(7),
                    label_override: r.try_get::<_, Option<String>>(8).ok().flatten(),
                    unit: r.try_get::<_, Option<String>>(9).ok().flatten(),
                    fmt: r.try_get::<_, Option<String>>(10).ok().flatten(),
                    reg_name: r.get::<_, String>(11),
                    reg_mb: r.get::<_, i32>(12),
                    reg_tip: r.get::<_, i32>(13),
                    reg_n_mb: r.get::<_, i32>(14),
                    reg_bits: r.try_get::<_, Option<i32>>(15).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }

    /// Function: $name.
    pub fn save_ui_window_bindings(&self, window_id: i64, bindings: &[UiWindowBindingRow]) -> Result<()> {
        self.rt.block_on(async {
            self.client.execute("begin", &[]).await?;
            self.client
                .execute(
                "delete from ui.kpz_window_reg_binding where window_id = $1",
                &[&window_id],
            )
            .await?;

            for b in bindings {
                if let Err(e) = self
                    .client
                    .execute(
                    "insert into ui.kpz_window_reg_binding(\
                        window_id, reg_id, pos, x, y, w, h, visible, writable, label_override, unit, fmt\
                     ) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
                    &[
                        &window_id,
                        &b.reg_id,
                        &b.pos,
                        &b.x,
                        &b.y,
                        &b.w,
                        &b.h,
                        &b.visible,
                        &b.writable,
                        &b.label_override,
                        &b.unit,
                        &b.fmt,
                    ],
                )
                .await
                {
                    let _ = self.client.execute("rollback", &[]).await;
                    return Err(e.into());
                }
            }
            self.client.execute("commit", &[]).await?;
            Ok(())
        })
    }

    /// Function: $name.
    pub fn get_regs_by_groups(&self, group_ids: &[i32]) -> Result<Vec<RegRow>> {
        if group_ids.is_empty() {
            return Ok(Vec::new());
        }
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, coalesce(name,''), coalesce(mb,0), coalesce(tip,0), bits \
                     from reg \
                     where grup = any($1) \
                     order by grup, mb asc nulls last, id asc",
                    &[&group_ids],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| RegRow {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                    mb: r.get::<_, i32>(2),
                    tip: r.get::<_, i32>(3),
                    bits: r.try_get::<_, Option<i32>>(4).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }
}

