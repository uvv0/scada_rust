use super::*;

/// Преобразует число в `f32` big-endian байты для поля `val_raw` в `arx_val`.
/// # Параметры
/// - `v`: числовое значение для преобразования/записи.
/// # Возвращает
/// - `Vec<u8>`: 4 байта значения в формате `f32` big-endian.
/// # Пример
/// - `let raw = f32_raw(value);`
pub(super) fn f32_raw(v: f64) -> Vec<u8> {
    let f = v as f32;
    f.to_be_bytes().to_vec()
}

/// Возвращает текущее Unix-время в секундах.
/// # Параметры
/// - Входные параметры отсутствуют.
/// # Возвращает
/// - `i64`: текущее Unix-время в секундах.
/// # Пример
/// - `let ts = now_unix();`
pub(super) fn now_unix() -> i64 {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    ms / 1000
}

/// Нормализует `emit.ts` к Unix-секундам (sec/ms/us/ns), валидирует диапазон и допустимое отклонение от текущего времени, иначе возвращает fallback.
/// # Параметры
/// - `ts`: временная метка из emit-скрипта.
/// - `fallback_unix`: резервное Unix-время при некорректной emit-временной метке.
/// # Возвращает
/// - `i64`: нормализованная Unix-временная метка или fallback.
/// # Пример
/// - `let ts = normalize_emit_ts_unix(ev.ts, now_unix());`
pub(super) fn normalize_emit_ts_unix(ts: f64, fallback_unix: i64) -> i64 {
    if !ts.is_finite() || ts <= 0.0 {
        return fallback_unix;
    }
    let raw = ts.round() as i64;
    // 2000-01-01 .. 2100-01-01 in unix seconds.
    const MIN_SEC: i64 = 946_684_800;
    const MAX_SEC: i64 = 4_102_444_800;

    // Reject timestamps far from "now": script emits are expected near current time.
    const MAX_SKEW_SEC: i64 = 366 * 24 * 3600;
    let accept = |candidate: i64| -> Option<i64> {
        if !(MIN_SEC..=MAX_SEC).contains(&candidate) {
            return None;
        }
        if (candidate - fallback_unix).abs() > MAX_SKEW_SEC {
            return None;
        }
        Some(candidate)
    };

    if let Some(v) = accept(raw) {
        return v;
    }
    let ms = raw / 1_000;
    if let Some(v) = accept(ms) {
        return v;
    }
    let us = raw / 1_000_000;
    if let Some(v) = accept(us) {
        return v;
    }
    let ns = raw / 1_000_000_000;
    if let Some(v) = accept(ns) {
        return v;
    }
    fallback_unix
}

/// Строит сетевые параметры соединения КПЗ (ip/port/rtu/modem) через строгую валидацию полей из БД.
/// # Параметры
/// - `kpz`: строка конфигурации КПЗ из БД.
/// - `obj_by_id`: справочник объектов по `obj.id`.
/// - `ip_by_id`: справочник IP-адресов по идентификатору item.
/// - `port_by_id`: справочник портов по идентификатору item.
/// # Возвращает
/// - `Result<ConnInfo>`: валидированные параметры соединения для опроса КПЗ.
/// # Пример
/// - `let conn = build_conn(&kpz, &obj_by_id, &ip_by_id, &port_by_id)?;`
pub(super) fn build_conn(
    kpz: &KpzRow,
    obj_by_id: &HashMap<i32, ObjRow>,
    ip_by_id: &HashMap<i32, String>,
    port_by_id: &HashMap<i32, u16>,
) -> Result<ConnInfo> {
    crate::db_queries::build_conn(kpz, obj_by_id, ip_by_id, port_by_id)
}

/// Декодирует битовую маску из 64 байт в список включенных групп (1-based).
/// # Параметры
/// - `grups`: битовая маска включенных групп (64 байта).
/// # Возвращает
/// - `Vec<i32>`: номера групп (1-based), включенные в битовой маске.
/// # Пример
/// - `let groups = decode_groups(&kpz.grups);`
pub(super) fn decode_groups(grups: &[u8]) -> Vec<i32> {
    if grups.len() != 64 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for bit in 0..(64 * 8) {
        let byte_index = bit >> 3;
        let bit_index = bit & 7;
        if (grups[byte_index] & (1 << bit_index)) != 0 {
            out.push((bit + 1) as i32);
        }
    }
    out
}

/// Нормализует период опроса в допустимый диапазон: `0` или `1..3600` секунд.
/// # Параметры
/// - `n`: значение периода/интервала перед нормализацией.
/// # Возвращает
/// - `i32`: нормализованное значение периода в допустимом диапазоне.
/// # Пример
/// - `let t_a = clamp_period(kpz.t_a);`
pub(super) fn clamp_period(n: i32) -> i32 {
    if n <= 0 {
        return 0;
    }
    if n < 1 {
        return 1;
    }
    if n > 3600 {
        return 3600;
    }
    n
}

/// Преобразует параметры объекта в UDP-параметры Modbus с дефолтами для `kanal/speed/stop/parit/bit`.
/// # Параметры
/// - `obj`: параметры объекта канала связи.
/// # Возвращает
/// - `HashMap<String,i32>`: параметры линии для формирования UDP Modbus-пакета.
/// # Пример
/// - `let udp = obj_to_udp(&obj);`
pub(super) fn obj_to_udp(obj: &ObjRow) -> HashMap<String, i32> {
    let mut m = HashMap::new();
    m.insert("kanal".to_string(), obj.kanal.unwrap_or(3));
    m.insert("speed".to_string(), obj.speed.unwrap_or(8));
    m.insert("stop".to_string(), obj.stop.unwrap_or(0));
    m.insert("parit".to_string(), obj.parit.unwrap_or(2));
    m.insert("bit".to_string(), obj.bit.unwrap_or(8));
    m
}

pub(super) struct GluedExec {
    pub multi: crate::modbus_service::ModbusResultMultiReq,
    pub dur_ms: i32,
}

impl SchedulerState {
    #[cfg(test)]
    pub(super) async fn exec_glued_reqs(
        &self,
        _client: &tokio_postgres::Client,
        transport: &UdpCorrelatedTransport,
        conn: &ConnInfo,
        reqs: &[ReadReq],
        timeout_ms: u64,
        group_id: i32,
        mode: &str,
        elam_rows: &mut Vec<ElamRow>,
    ) -> Result<GluedExec> {
        let idle_ms = ((reqs.len() as u64) * 25).clamp(60, 500);
        let sw = std::time::Instant::now();
        let multi = match request_reqs_glued(
            transport,
            &obj_to_udp(&self.obj_by_id[&conn.obj_id]),
            &conn.ip,
            conn.port,
            conn.rtu,
            conn.modem,
            reqs,
            conn.max_pkt_len as usize,
            Duration::from_millis(timeout_ms),
            Duration::from_millis(idle_ms),
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                let dur_ms = sw.elapsed().as_millis() as i32;
                let msg = format!("ERROR: transport: {}", e);
                elam_rows.push(build_elam_transport_error_row(conn, group_id, dur_ms, &msg));
                let _ = mode;
                return Err(e);
            }
        };

        let dur_ms = sw.elapsed().as_millis() as i32;
        Ok(GluedExec { multi, dur_ms })
    }
}

impl WorkerCtx {
    pub(super) async fn exec_glued_reqs(
        &self,
        client: &tokio_postgres::Client,
        transport: &UdpCorrelatedTransport,
        conn: &ConnInfo,
        reqs: &[ReadReq],
        timeout_ms: u64,
        group_id: i32,
        mode: &str,
        elam_rows: &mut Vec<ElamRow>,
    ) -> Result<GluedExec> {
        let idle_ms = ((reqs.len() as u64) * 25).clamp(60, 500);
        let sw = std::time::Instant::now();
        let multi = match request_reqs_glued(
            transport,
            &obj_to_udp(&self.shared.obj_by_id[&conn.obj_id]),
            &conn.ip,
            conn.port,
            conn.rtu,
            conn.modem,
            reqs,
            conn.max_pkt_len as usize,
            Duration::from_millis(timeout_ms),
            Duration::from_millis(idle_ms),
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                let dur_ms = sw.elapsed().as_millis() as i32;
                let msg = format!("ERROR: transport: {}", e);
                elam_rows.push(build_elam_transport_error_row(conn, group_id, dur_ms, &msg));
                if let Err(ins_e) = insert_elam_rows(client, elam_rows).await {
                    tracing::warn!(
                        kpz_id = conn.kpz_id,
                        group_id = group_id,
                        mode = mode,
                        rows = elam_rows.len(),
                        err = %ins_e,
                        "failed to batch insert elam rows"
                    );
                }
                return Err(e);
            }
        };

        let dur_ms = sw.elapsed().as_millis() as i32;
        Ok(GluedExec { multi, dur_ms })
    }
}

/// Возвращает базу scoped-ключей RV для конкретного КПЗ, чтобы изолировать служебные диапазоны между устройствами.
/// # Параметры
/// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
/// # Возвращает
/// - `i64`: базовый scoped-ключ RV для данного КПЗ.
/// # Пример
/// - `let base = svc_base_for(kpz_id);`
pub(super) fn svc_base_for(kpz_id: i32) -> i64 {
    1_000_000_000i64 + (kpz_id as i64) * 200_000i64
}

/// Формирует scoped RV-ключ как `svc_base_for(kpz_id) + offset`.
/// # Параметры
/// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
/// - `offset`: смещение внутри scoped-служебного диапазона ключей RV.
/// # Возвращает
/// - `i64`: scoped RV-ключ для заданного смещения.
/// # Пример
/// - `let key = svc_key(kpz_id, 80000);`
pub(super) fn svc_key(kpz_id: i32, offset: i32) -> i64 {
    svc_base_for(kpz_id) + (offset as i64)
}

/// Формирует scoped RV-ключ для ARX индекса (`40000 + arx_id`).
/// # Параметры
/// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
/// - `arx_id`: идентификатор ARX-индекса.
/// # Возвращает
/// - `i64`: scoped RV-ключ для ARX-индекса.
/// # Пример
/// - `let key = svc_arx_key(kpz_id, arx_id);`
pub(super) fn svc_arx_key(kpz_id: i32, arx_id: i32) -> i64 {
    svc_base_for(kpz_id) + 40000i64 + (arx_id as i64)
}

/// Сравнивает предыдущее и новое значение с учетом `NaN/Inf` и эпсилон-порога, чтобы отфильтровать логический шум.
/// # Параметры
/// - `prev`: предыдущее значение для сравнения изменения.
/// - `new_v`: новое значение для сравнения изменения.
/// # Возвращает
/// - `bool`: `true`, если значение считается изменившимся.
/// # Пример
/// - `let changed = value_changed(prev, new_v);`
pub(super) fn value_changed(prev: Option<f64>, new_v: f64) -> bool {
    let Some(prev_v) = prev else {
        return true;
    };
    if !prev_v.is_finite() || !new_v.is_finite() {
        return prev_v.to_bits() != new_v.to_bits();
    }
    (prev_v - new_v).abs() > 1e-9
}

/// Вычисляет детерминированный сдвиг фазы внутри периода для равномерного распределения нагрузки между КПЗ.
/// # Параметры
/// - `period`: базовый период задания для вычисления фазового сдвига.
/// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
/// - `kind_salt`: соль типа задания (A/Script) для независимого фазового сдвига.
/// # Возвращает
/// - `Duration`: детерминированный фазовый сдвиг внутри периода.
/// # Пример
/// - `let off = phase_offset(Duration::from_secs(1), kpz_id, 0);`
pub(super) fn phase_offset(period: Duration, kpz_id: i32, kind_salt: u64) -> Duration {
    let p = period.as_millis() as u64;
    if p == 0 {
        return Duration::from_millis(0);
    }
    let mut x = (kpz_id as i64 as u64) ^ (kind_salt.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^= x >> 33;
    Duration::from_millis(x % p)
}

/// Извлекает `packet_id` и `pkt_type` из запроса/ответа для ELAM-логирования.
/// # Параметры
/// - `req`: байты исходного UDP/Modbus запроса.
/// - `resp`: необязательные байты ответа, если ответ получен.
/// # Возвращает
/// - `(i32, i32)`: пара `(packet_id, pkt_type)` для трассировки ELAM.
/// # Пример
/// - `let (packet_id, pkt_type) = pkt_meta(&req, resp.as_deref());`
pub(super) fn pkt_meta(req: &[u8], resp: Option<&[u8]>) -> (i32, i32) {
    let packet_id = req.get(3).copied().unwrap_or(0) as i32;
    let pkt_type = resp
        .and_then(|r| r.get(4).copied())
        .or_else(|| req.get(4).copied())
        .unwrap_or(0) as i32;
    (packet_id, pkt_type)
}

/// Форматирует HEX-превью байтового массива с ограничением длины и суффиксом `...` при усечении.
/// # Параметры
/// - `data`: буфер байт для HEX-предпросмотра.
/// - `max_bytes`: максимум байт, включаемых в HEX-предпросмотр.
/// # Возвращает
/// - `String`: форматированная HEX-строка с возможным усечением.
/// # Пример
/// - `let hex = hex_preview(&tx, 256);`
pub(super) fn hex_preview(data: &[u8], max_bytes: usize) -> String {
    let n = std::cmp::min(data.len(), max_bytes);
    let mut out = String::new();
    for (i, b) in data.iter().take(n).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{:02X}", b);
    }
    if data.len() > max_bytes {
        out.push_str(" ...");
    }
    out
}

/// Формирует детальную ELAM-строку по одному Modbus-запросу/ответу; пропускает запись, если логирование статуса отключено.
/// # Параметры
/// - `conn`: сетевые и протокольные параметры подключения КПЗ (ip/port/rtu/modem и т.д.).
/// - `group_id`: идентификатор группы регистров/скриптов.
/// - `res`: результат одного Modbus-запроса из glued-пакета.
/// - `req_bytes`: полный байтовый запрос для записи в ELAM.
/// - `duration_ms`: длительность обмена в миллисекундах.
/// - `status`: итоговый статус обмена/обработки для логирования.
/// # Возвращает
/// - `Option<ElamRow>`: строка ELAM для конкретного запроса или `None`.
/// # Пример
/// - `let row = build_elam_row(&conn, group_id, res, &req, dur_ms, &status);`
pub(super) fn build_elam_row(
    conn: &ConnInfo,
    group_id: i32,
    res: &crate::modbus_service::ModbusResultReq,
    req_bytes: &[u8],
    duration_ms: i32,
    status: &str,
) -> Option<ElamRow> {
    if !should_log_elam(status) {
        return None;
    }
    let (packet_id, pkt_type) = pkt_meta(req_bytes, res.response.as_deref());
    let err = if status.starts_with("ERROR") {
        Some(status.to_string())
    } else {
        None
    };
    Some(ElamRow {
        kpz_id: conn.kpz_id,
        group_id,
        packet_id,
        pkt_type,
        func: res.func as i32,
        addr_human: res.addr_human,
        count_words: res.cnt_words,
        req: req_bytes.to_vec(),
        resp: res.response.clone(),
        duration_ms,
        status: status.to_string(),
        err,
    })
}

/// Формирует SUMMARY-строку ELAM по пакету glued-запроса с фиксацией ожидаемого/полученного количества ответов и текста рассинхронизации.
/// # Параметры
/// - `conn`: сетевые и протокольные параметры подключения КПЗ (ip/port/rtu/modem и т.д.).
/// - `group_id`: идентификатор группы регистров/скриптов.
/// - `req_bytes`: полный байтовый запрос для записи в ELAM.
/// - `resp_bytes`: необязательный полный байтовый ответ для записи в ELAM.
/// - `duration_ms`: длительность обмена в миллисекундах.
/// - `expected`: ожидаемое число ответов в glued-пакете.
/// - `received`: фактическое число полученных ответов.
/// # Возвращает
/// - `ElamRow`: summary-запись ELAM по glued-пакету.
/// # Пример
/// - `let row = build_elam_summary_row(&conn, group_id, &req, resp.as_deref(), dur_ms, expected, received);`
pub(super) fn build_elam_summary_row(
    conn: &ConnInfo,
    group_id: i32,
    req_bytes: &[u8],
    resp_bytes: Option<&[u8]>,
    duration_ms: i32,
    expected: usize,
    received: usize,
) -> ElamRow {
    let (packet_id, pkt_type) = pkt_meta(req_bytes, resp_bytes);
    let missing = expected.saturating_sub(received);
    let (status, err) = if received == expected {
        (
            format!("SUMMARY: responses == commands ({}/{})", received, expected),
            None,
        )
    } else {
        let msg = format!(
            "SUMMARY: responses < commands ({}/{}), missing={}",
            received, expected, missing
        );
        (msg.clone(), Some(msg))
    };

    ElamRow {
        kpz_id: conn.kpz_id,
        group_id,
        packet_id,
        pkt_type,
        func: 0,
        addr_human: expected as i32,
        count_words: received as i32,
        req: req_bytes.to_vec(),
        resp: resp_bytes.map(|v| v.to_vec()),
        duration_ms,
        status,
        err,
    }
}

/// Формирует ELAM-запись для ошибок транспортного обмена до получения ответа.
pub(super) fn build_elam_transport_error_row(
    conn: &ConnInfo,
    group_id: i32,
    duration_ms: i32,
    err_text: &str,
) -> ElamRow {
    ElamRow {
        kpz_id: conn.kpz_id,
        group_id,
        packet_id: 0,
        pkt_type: 0,
        func: 0,
        addr_human: 0,
        count_words: 0,
        req: Vec::new(),
        resp: None,
        duration_ms,
        status: err_text.to_string(),
        err: Some(err_text.to_string()),
    }
}

/// Политика логирования ELAM по статусу обмена (сейчас пропускает все статусы).
/// # Параметры
/// - `status`: итоговый статус обмена/обработки для логирования.
/// # Возвращает
/// - `bool`: признак, следует ли писать ELAM для данного статуса.
/// # Пример
/// - `if should_log_elam(&status) { /* insert elam */ }`
pub(super) fn should_log_elam(status: &str) -> bool {
    if status.starts_with("ERROR") || status.starts_with("WARN") {
        return true;
    }
    if status == "OK" {
        return true;
    }
    true
}

/// Вычисляет активность alarm-правила по типу сравнения (`lt/le/gt/ge/.../between/outside`) с учетом hysteresis для удержания состояния.
/// # Параметры
/// - `rule`: правило alarm с порогами, задержками и гистерезисом.
/// - `value`: новое числовое значение регистра для проверки/обновления.
/// - `currently_active`: текущее состояние alarm перед пересчетом.
/// # Возвращает
/// - `bool`: расчетное состояние alarm с учетом порогов и hysteresis.
/// # Пример
/// - `let active = alarm_should_be_active(&rule, value, currently_active);`
pub(super) fn alarm_should_be_active(rule: &AlarmRule, value: f64, currently_active: bool) -> bool {
    let cmp = rule.cmp.as_str();
    let lo = rule.set_lo.unwrap_or(0.0);
    let hi = rule.set_hi.unwrap_or(0.0);
    let lo_1 = rule.set_lo_1.unwrap_or(lo);
    let hi_1 = rule.set_hi_1.unwrap_or(hi);
    let h = if rule.hysteresis.is_finite() && rule.hysteresis > 0.0 {
        rule.hysteresis
    } else {
        0.0
    };

    match cmp {
        "lt" => {
            if currently_active {
                value < (lo + h)
            } else {
                value < lo
            }
        }
        "le" => {
            if currently_active {
                value <= (lo + h)
            } else {
                value <= lo
            }
        }
        "gt" => {
            if currently_active {
                value > (hi - h)
            } else {
                value > hi
            }
        }
        "ge" => {
            if currently_active {
                value >= (hi - h)
            } else {
                value >= hi
            }
        }
        "lt_1" => {
            if currently_active {
                value < (lo_1 + h)
            } else {
                value < lo_1
            }
        }
        "le_1" => {
            if currently_active {
                value <= (lo_1 + h)
            } else {
                value <= lo_1
            }
        }
        "gt_1" => {
            if currently_active {
                value > (hi_1 - h)
            } else {
                value > hi_1
            }
        }
        "ge_1" => {
            if currently_active {
                value >= (hi_1 - h)
            } else {
                value >= hi_1
            }
        }
        "between" => value >= lo && value <= hi,
        "outside" => value < lo || value > hi,
        _ => false,
    }
}
