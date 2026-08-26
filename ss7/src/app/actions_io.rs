use std::collections::{BTreeMap, HashMap};
use std::thread;
use std::time::Duration;

use crate::app::{CmdWorkerResult, IoTaskResult, PollNowWorkerResult, Ss7App};
use crate::app_io::{
    hex_join, parse_read_words_from_resp, send_mb_over_udp, validate_modbus_response, IoConn,
};
use crate::app_windows::parse_binding_value_from_words_static;
use crate::modbus;
use crate::modbus_service::{request_reqs_glued, ReadReq, ServiceConn};
use crate::models::UiWindowBindingRow;

pub(crate) fn read_func_by_n_mb_from_map(ref_n_mb: &HashMap<i32, String>, n_mb_id: i32) -> u8 {
    let name = ref_n_mb
        .get(&n_mb_id)
        .map(|s| s.trim().to_uppercase())
        .unwrap_or_default();
    if name.contains("TIT") {
        4
    } else {
        3
    }
}

impl Ss7App {
    pub(crate) fn read_func_by_n_mb(&self, n_mb_id: i32) -> u8 {
        let name = self
            .ref_n_mb
            .get(&n_mb_id)
            .map(|s| s.trim().to_uppercase())
            .unwrap_or_default();
        if name.contains("TIT") {
            4
        } else {
            3
        }
    }

    #[allow(dead_code)]
    fn read_binding_direct(&self, conn: &IoConn, b: &crate::models::UiWindowBindingRow) -> Result<f64, String> {
        let (v, _tx, _rx, _func) = self.read_binding_direct_with_packets(conn, b)?;
        Ok(v)
    }

    pub(crate) fn parse_binding_value_from_words(
        &self,
        b: &crate::models::UiWindowBindingRow,
        words: &[u16],
    ) -> Result<f64, String> {
        if words.is_empty() {
            return Err("empty words".to_string());
        }
        if matches!(b.reg_tip, 2 | 4 | 5) {
            if words.len() < 2 {
                return Err("not enough words for 32-bit value".to_string());
            }
            let hi = words[0];
            let lo = words[1];
            let bytes = [
                ((hi >> 8) & 0xFF) as u8,
                (hi & 0xFF) as u8,
                ((lo >> 8) & 0xFF) as u8,
                (lo & 0xFF) as u8,
            ];
            let v = match b.reg_tip {
                5 => f32::from_be_bytes(bytes) as f64,
                4 => u32::from_be_bytes(bytes) as f64,
                2 => i32::from_be_bytes(bytes) as f64,
                _ => u32::from_be_bytes(bytes) as f64,
            };
            return Ok(v);
        }
        if b.reg_tip == 0
            && let Some(bit) = b.reg_bits
            && (0..=15).contains(&bit)
        {
            return Ok(((words[0] >> bit) & 1) as f64);
        }
        let w = words[0];
        let v = match b.reg_tip {
            1 => (w as i16) as f64,
            _ => w as f64,
        };
        Ok(v)
    }

    #[allow(dead_code)]
    pub(crate) fn read_binding_direct_with_packets(
        &self,
        conn: &IoConn,
        b: &crate::models::UiWindowBindingRow,
    ) -> Result<(f64, Vec<u8>, Vec<u8>, u8), String> {
        let func = self.read_func_by_n_mb(b.reg_n_mb);
        let words_cnt: u16 = if matches!(b.reg_tip, 2 | 4 | 5) { 2 } else { 1 };
        let mb = modbus::sout_mb_only(conn.rtu, func, b.reg_mb, words_cnt, None)?;
        let (tx, resp_res) = send_mb_over_udp(conn, &mb, Duration::from_millis(conn.timeout_ms));
        let resp = resp_res.map_err(|e| format!("{e}; tx=[{}]", hex_join(&tx)))?;
        let words = parse_read_words_from_resp(&resp, func)?;
        let v = self.parse_binding_value_from_words(b, &words)?;
        Ok((v, tx, resp, func))
    }

    pub(crate) fn ui_link_poll_now(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(kpz_id) = self.selected_kpz else {
            self.ui_link_editor.err = Some("Select KPZ first".to_string());
            return;
        };
        let bindings = self.ui_link_editor.bindings.clone();
        let ids: Vec<i32> = bindings
            .iter()
            .filter(|b| !(b.is_text || b.reg_id <= 0))
            .map(|b| b.reg_id)
            .collect();
        let conn_res = self.build_io_conn();
        let ref_n_mb = self.ref_n_mb.clone();

        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("polling in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let mut live_values: BTreeMap<i32, Option<f64>> = BTreeMap::new();
            let mut trace_lines: Vec<String> = Vec::new();
            let mut ok = 0usize;
            let mut fail = 0usize;
            let mut first_err: Option<String> = None;

            let poll_bindings: Vec<_> = bindings
                .into_iter()
                .filter(|b| !(b.is_text || b.reg_id <= 0) && !(b.reg_n_mb == 1 || b.reg_tip == 1))
                .collect();

            match conn_res {
                Ok(conn) => {
                    let mut by_func: BTreeMap<u8, Vec<UiWindowBindingRow>> = BTreeMap::new();
                    for b in poll_bindings {
                        by_func
                            .entry(read_func_by_n_mb_from_map(&ref_n_mb, b.reg_n_mb))
                            .or_default()
                            .push(b);
                    }
                    let mut blocks: Vec<(u8, i32, i32, Vec<UiWindowBindingRow>)> = Vec::new();
                    for (func, mut regs) in by_func {
                        regs.sort_by_key(|b| b.reg_mb);
                        let mut cur: Vec<UiWindowBindingRow> = Vec::new();
                        let mut start = 0i32;
                        let mut end = 0i32;
                        for b in regs {
                            let w = if matches!(b.reg_tip, 2 | 4 | 5) { 2 } else { 1 };
                            let rs = b.reg_mb;
                            let re = b.reg_mb + w - 1;
                            if cur.is_empty() {
                                start = rs;
                                end = re;
                                cur.push(b);
                                continue;
                            }
                            let next_end = end.max(re);
                            let next_words = next_end - start + 1;
                            if rs <= end + 1 && next_words <= 120 {
                                end = next_end;
                                cur.push(b);
                            } else {
                                blocks.push((func, start, end - start + 1, cur));
                                start = rs;
                                end = re;
                                cur = vec![b];
                            }
                        }
                        if !cur.is_empty() {
                            blocks.push((func, start, end - start + 1, cur));
                        }
                    }
                    let reqs: Vec<ReadReq> = blocks
                        .iter()
                        .map(|(func, addr, cnt, _)| ReadReq {
                            func: *func,
                            addr_human: *addr,
                            cnt_words: *cnt,
                        })
                        .collect();
                    if reqs.is_empty() {
                        let _ = tx.send(IoTaskResult::PollNow(PollNowWorkerResult {
                            live_values,
                            poll_trace: String::new(),
                            status: Some("no poll requests".to_string()),
                            err: None,
                        }));
                        return;
                    }

                    let idle_ms = ((reqs.len() as u64) * 25).clamp(60, 500);
                    let service_conn = ServiceConn {
                        ip: conn.ip.clone(),
                        port: conn.port,
                        rtu: conn.rtu,
                        modem: conn.modem,
                        kan: conn.kan,
                        speed: conn.speed,
                        stop: conn.stop,
                        par: conn.par,
                        data: conn.data,
                        max_pkt_len: conn.max_pkt_len,
                    };
                    match request_reqs_glued(
                        &service_conn,
                        &reqs,
                        Duration::from_millis(conn.timeout_ms),
                        Duration::from_millis(idle_ms),
                    ) {
                        Ok(multi) => {
                            trace_lines.extend(multi.trace_lines);
                            for (i, (func, addr, _cnt, regs)) in blocks.iter().enumerate() {
                                let Some(res) = multi.results.get(i) else {
                                    for b in regs {
                                        live_values.insert(b.reg_id, None);
                                        fail += 1;
                                        let msg = format!("reg {} mb {}: no response", b.reg_id, b.reg_mb);
                                        trace_lines.push(format!("ERR {}", msg));
                                        if first_err.is_none() {
                                            first_err = Some(msg);
                                        }
                                    }
                                    continue;
                                };
                                let Some(vpkt) = &res.response else {
                                    for b in regs {
                                        live_values.insert(b.reg_id, None);
                                        fail += 1;
                                        let msg = format!("reg {} mb {}: empty response", b.reg_id, b.reg_mb);
                                        trace_lines.push(format!("ERR {}", msg));
                                        if first_err.is_none() {
                                            first_err = Some(msg);
                                        }
                                    }
                                    continue;
                                };
                                match parse_read_words_from_resp(vpkt, *func) {
                                    Ok(words) => {
                                        let rx_mb_hex = modbus::extract_modbus_frame(vpkt)
                                            .map(hex_join)
                                            .unwrap_or_else(|| "<no-mb>".to_string());
                                        trace_lines.push(format!(
                                            "BLOCK fc={} addr={} regs={} words={} rx_mb=[{}]",
                                            func,
                                            addr,
                                            regs.len(),
                                            words.len(),
                                            rx_mb_hex
                                        ));
                                        for b in regs {
                                            let off = (b.reg_mb - *addr) as usize;
                                            let need = if matches!(b.reg_tip, 2 | 4 | 5) { 2 } else { 1 };
                                            if off + need > words.len() {
                                                live_values.insert(b.reg_id, None);
                                                fail += 1;
                                                let msg = format!(
                                                    "reg {} mb {}: out of block bounds off={} need={} words={}",
                                                    b.reg_id, b.reg_mb, off, need, words.len()
                                                );
                                                trace_lines.push(format!("ERR {}", msg));
                                                if first_err.is_none() {
                                                    first_err = Some(msg);
                                                }
                                                continue;
                                            }
                                            match parse_binding_value_from_words_static(b, &words[off..off + need]) {
                                                Ok(v) => {
                                                    live_values.insert(b.reg_id, Some(v));
                                                    ok += 1;
                                                    trace_lines.push(format!(
                                                        "OK  reg={} mb={} fc={} off={} need={} val={}",
                                                        b.reg_id, b.reg_mb, func, off, need, v
                                                    ));
                                                }
                                                Err(e) => {
                                                    live_values.insert(b.reg_id, None);
                                                    fail += 1;
                                                    let msg = format!("reg {} mb {}: {}", b.reg_id, b.reg_mb, e);
                                                    trace_lines.push(format!("ERR {}", msg));
                                                    if first_err.is_none() {
                                                        first_err = Some(msg);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        for b in regs {
                                            live_values.insert(b.reg_id, None);
                                            fail += 1;
                                            let msg = format!("reg {} mb {}: {}", b.reg_id, b.reg_mb, e);
                                            trace_lines.push(format!("ERR {}", msg));
                                            if first_err.is_none() {
                                                first_err = Some(msg);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            for (_func, _addr, _cnt, regs) in &blocks {
                                for b in regs {
                                    live_values.insert(b.reg_id, None);
                                    fail += 1;
                                    let msg = format!("reg {} mb {}: {}", b.reg_id, b.reg_mb, e);
                                    trace_lines.push(format!("ERR {}", msg));
                                    if first_err.is_none() {
                                        first_err = Some(msg.clone());
                                    }
                                }
                            }
                        }
                    }
                }
                Err(io_err) => {
                    first_err = Some(format!("IO unavailable: {io_err}"));
                    fail = ids.len();
                }
            }

            if fail == 0 {
                let _ = tx.send(IoTaskResult::PollNow(PollNowWorkerResult {
                    live_values,
                    poll_trace: trace_lines.join("\n"),
                    status: Some(format!("poll IO done: ok={ok}")),
                    err: None,
                }));
                return;
            }

            let (status, err) = match crate::db::Db::connect_from_env()
                .and_then(|db| db.get_reg_live_values(kpz_id, &ids))
            {
                Ok(rows) => {
                    for (id, v) in rows {
                        match live_values.get(&id).copied() {
                            Some(Some(_)) => {}
                            Some(None) => {
                                live_values.insert(id, v);
                            }
                            None => {
                                live_values.insert(id, v);
                            }
                        }
                    }
                    (
                        Some(format!("poll IO partial: ok={ok}, fail={fail}, DB fallback used")),
                        first_err,
                    )
                }
                Err(de) => (
                    Some(format!("poll IO partial: ok={ok}, fail={fail}")),
                    Some(format!("{}; db fallback failed: {de}", first_err.unwrap_or_default())),
                ),
            };
            let _ = tx.send(IoTaskResult::PollNow(PollNowWorkerResult {
                live_values,
                poll_trace: trace_lines.join("\n"),
                status,
                err,
            }));
        });
    }

    pub(crate) fn ui_link_send_tu(&mut self, reg_id: i32, on: bool) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(binding) = self.ui_link_editor.bindings.iter().find(|b| b.reg_id == reg_id).cloned() else {
            self.ui_link_editor.err = Some(format!("binding for reg {} not found", reg_id));
            return;
        };
        if !(binding.reg_n_mb == 1 || binding.reg_tip == 1) {
            self.ui_link_editor.err = Some(format!(
                "reg {} is not TU command (n_mb={}, tip={})",
                reg_id, binding.reg_n_mb, binding.reg_tip
            ));
            return;
        }

        let conn = match self.build_io_conn() {
            Ok(c) => c,
            Err(e) => {
                self.ui_link_editor.err = Some(e);
                return;
            }
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("send TU in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let dat = if on { [0xFFu8, 0x00u8] } else { [0x00u8, 0x00u8] };
            let mb = match modbus::sout_mb_only(conn.rtu, 5, binding.reg_mb, 1, Some(&dat)) {
                Ok(v) => v,
                Err(e) => {
                    let _ = tx.send(IoTaskResult::SendTu(CmdWorkerResult {
                        reg_id,
                        live_value: None,
                        last_cmd: "ERR".to_string(),
                        status: None,
                        err: Some(format!("fc5 build failed: {e}")),
                    }));
                    return;
                }
            };
            let (tx_bytes, resp_res) = send_mb_over_udp(&conn, &mb, Duration::from_millis(conn.timeout_ms));
            let msg = match resp_res {
                Ok(resp) => match validate_modbus_response(&resp, 5) {
                    Ok(()) => CmdWorkerResult {
                        reg_id,
                        live_value: None,
                        last_cmd: if on { "OK ON".to_string() } else { "OK OFF".to_string() },
                        status: Some(format!(
                            "FC5 {} OK reg={} mb={} ip={}:{} rtu={} tx=[{}]",
                            if on { "ON" } else { "OFF" },
                            reg_id,
                            binding.reg_mb,
                            conn.ip,
                            conn.port,
                            conn.rtu,
                            hex_join(&tx_bytes)
                        )),
                        err: None,
                    },
                    Err(e) => CmdWorkerResult {
                        reg_id,
                        live_value: None,
                        last_cmd: "ERR".to_string(),
                        status: None,
                        err: Some(format!(
                            "fc5 bad response reg={} mb={} ip={}:{} rtu={} err={} tx=[{}]",
                            reg_id,
                            binding.reg_mb,
                            conn.ip,
                            conn.port,
                            conn.rtu,
                            e,
                            hex_join(&tx_bytes)
                        )),
                    },
                },
                Err(e) => CmdWorkerResult {
                    reg_id,
                    live_value: None,
                    last_cmd: "ERR".to_string(),
                    status: None,
                    err: Some(format!(
                        "fc5 send failed reg={} mb={} ip={}:{} rtu={} err={} tx=[{}]",
                        reg_id,
                        binding.reg_mb,
                        conn.ip,
                        conn.port,
                        conn.rtu,
                        e,
                        hex_join(&tx_bytes)
                    )),
                },
            };
            let _ = tx.send(IoTaskResult::SendTu(msg));
        });
    }

    pub(crate) fn ui_link_write_value(&mut self, reg_id: i32, val: f64) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(binding) = self.ui_link_editor.bindings.iter().find(|b| b.reg_id == reg_id).cloned() else {
            self.ui_link_editor.err = Some(format!("binding for reg {} not found", reg_id));
            return;
        };
        if binding.reg_n_mb == 1 || binding.reg_tip == 1 {
            self.ui_link_editor.err = Some(format!(
                "reg {} is TU command (n_mb={}, tip={}), use FC5 ON/OFF",
                reg_id, binding.reg_n_mb, binding.reg_tip
            ));
            return;
        }
        if !binding.writable {
            self.ui_link_editor.err = Some(format!("reg {} is not writable", reg_id));
            return;
        }
        let conn = match self.build_io_conn() {
            Ok(c) => c,
            Err(e) => {
                self.ui_link_editor.err = Some(e);
                return;
            }
        };
        let ref_n_mb = self.ref_n_mb.clone();
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("write in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let res = write_value_worker(reg_id, val, binding, conn, &ref_n_mb);
            let _ = tx.send(IoTaskResult::WriteValue(res));
        });
    }
}

pub(crate) fn write_value_worker(
    reg_id: i32,
    val: f64,
    binding: UiWindowBindingRow,
    conn: IoConn,
    ref_n_mb: &HashMap<i32, String>,
) -> CmdWorkerResult {
    if binding.reg_tip == 0
        && let Some(bit) = binding.reg_bits
        && (0..=15).contains(&bit)
    {
        let read_func = read_func_by_n_mb_from_map(ref_n_mb, binding.reg_n_mb);
        let read_mb = match modbus::sout_mb_only(conn.rtu, read_func, binding.reg_mb, 1, None) {
            Ok(v) => v,
            Err(e) => {
                return CmdWorkerResult {
                    reg_id,
                    live_value: None,
                    last_cmd: "ERR".to_string(),
                    status: None,
                    err: Some(format!("bit read build failed: {e}")),
                };
            }
        };
        let (_, read_resp_res) = send_mb_over_udp(&conn, &read_mb, Duration::from_millis(conn.timeout_ms));
        let read_resp = match read_resp_res {
            Ok(v) => v,
            Err(e) => {
                return CmdWorkerResult {
                    reg_id,
                    live_value: None,
                    last_cmd: "ERR".to_string(),
                    status: None,
                    err: Some(format!("bit read failed for reg {}: {e}", reg_id)),
                };
            }
        };
        let words = match parse_read_words_from_resp(&read_resp, read_func) {
            Ok(v) => v,
            Err(e) => {
                return CmdWorkerResult {
                    reg_id,
                    live_value: None,
                    last_cmd: "ERR".to_string(),
                    status: None,
                    err: Some(format!("bit read parse failed for reg {}: {e}", reg_id)),
                };
            }
        };
        let cur = words.first().copied().unwrap_or(0);
        let mask = 1u16 << (bit as u16);
        let on = val >= 0.5;
        let new_word = if on { cur | mask } else { cur & !mask };
        let dat = [((new_word >> 8) & 0xFF) as u8, (new_word & 0xFF) as u8];
        let write6_mb = match modbus::sout_mb_only(conn.rtu, 6, binding.reg_mb, 1, Some(&dat)) {
            Ok(v) => v,
            Err(e) => {
                return CmdWorkerResult {
                    reg_id,
                    live_value: None,
                    last_cmd: "ERR".to_string(),
                    status: None,
                    err: Some(format!("bit write build failed (fc6): {e}")),
                };
            }
        };
        let (_, write6_resp_res) =
            send_mb_over_udp(&conn, &write6_mb, Duration::from_millis(conn.timeout_ms));
        let mut ok = false;
        let mut write_err = String::new();
        match write6_resp_res {
            Ok(resp) => match validate_modbus_response(&resp, 6) {
                Ok(()) => ok = true,
                Err(e) => write_err = format!("fc6 bad response: {e}"),
            },
            Err(e) => write_err = format!("fc6 send failed: {e}"),
        }
        if !ok {
            let write16_mb = match modbus::sout_mb_only(conn.rtu, 16, binding.reg_mb, 1, Some(&dat)) {
                Ok(v) => v,
                Err(e) => {
                    return CmdWorkerResult {
                        reg_id,
                        live_value: None,
                        last_cmd: "ERR".to_string(),
                        status: None,
                        err: Some(format!("bit write build failed (fc16): {e}")),
                    };
                }
            };
            let (_, write16_resp_res) =
                send_mb_over_udp(&conn, &write16_mb, Duration::from_millis(conn.timeout_ms));
            match write16_resp_res {
                Ok(resp) => match validate_modbus_response(&resp, 16) {
                    Ok(()) => ok = true,
                    Err(e) => write_err = format!("fc16 bad response: {e}; prev={write_err}"),
                },
                Err(e) => write_err = format!("fc16 send failed: {e}; prev={write_err}"),
            }
        }
        if ok {
            return CmdWorkerResult {
                reg_id,
                live_value: Some(if on { 1.0 } else { 0.0 }),
                last_cmd: "OK BIT".to_string(),
                status: Some(format!(
                    "BIT write: reg {} mb={} bit={} {} word=0x{:04X}",
                    reg_id,
                    binding.reg_mb,
                    bit,
                    if on { "ON" } else { "OFF" },
                    new_word
                )),
                err: None,
            };
        }
        return CmdWorkerResult {
            reg_id,
            live_value: None,
            last_cmd: "ERR".to_string(),
            status: None,
            err: Some(format!("bit write failed for reg {}: {}", reg_id, write_err)),
        };
    }
    if matches!(binding.reg_tip, 2 | 4 | 5) {
        let bytes = match binding.reg_tip {
            5 => (val as f32).to_be_bytes().to_vec(),
            4 => (val.max(0.0) as u32).to_be_bytes().to_vec(),
            2 => (val as i32).to_be_bytes().to_vec(),
            _ => (val as i32).to_be_bytes().to_vec(),
        };
        let dat = vec![bytes[0], bytes[1], bytes[2], bytes[3]];
        let mb = match modbus::sout_mb_only(conn.rtu, 16, binding.reg_mb, 2, Some(&dat)) {
            Ok(v) => v,
            Err(e) => {
                return CmdWorkerResult {
                    reg_id,
                    live_value: None,
                    last_cmd: "ERR".to_string(),
                    status: None,
                    err: Some(format!("fc16 build failed: {e}")),
                };
            }
        };
        let (_, resp_res) = send_mb_over_udp(&conn, &mb, Duration::from_millis(conn.timeout_ms));
        return match resp_res {
            Ok(resp) => match validate_modbus_response(&resp, 16) {
                Ok(()) => CmdWorkerResult {
                    reg_id,
                    live_value: Some(val),
                    last_cmd: "OK FC16".to_string(),
                    status: Some(format!(
                        "FC16 sent: reg {} mb={} value={:.3}",
                        reg_id, binding.reg_mb, val
                    )),
                    err: None,
                },
                Err(e) => CmdWorkerResult {
                    reg_id,
                    live_value: None,
                    last_cmd: "ERR".to_string(),
                    status: None,
                    err: Some(format!("fc16 bad response for reg {}: {e}", reg_id)),
                },
            },
            Err(e) => CmdWorkerResult {
                reg_id,
                live_value: None,
                last_cmd: "ERR".to_string(),
                status: None,
                err: Some(format!("fc16 send failed for reg {}: {e}", reg_id)),
            },
        };
    }

    let w = if binding.reg_tip == 1 {
        (val as i16) as u16
    } else {
        val.max(0.0) as u16
    };
    let dat = [((w >> 8) & 0xFF) as u8, (w & 0xFF) as u8];
    let mb6 = match modbus::sout_mb_only(conn.rtu, 6, binding.reg_mb, 1, Some(&dat)) {
        Ok(v) => v,
        Err(e) => {
            return CmdWorkerResult {
                reg_id,
                live_value: None,
                last_cmd: "ERR".to_string(),
                status: None,
                err: Some(format!("fc6 build failed: {e}")),
            };
        }
    };
    let (_, resp6_res) = send_mb_over_udp(&conn, &mb6, Duration::from_millis(conn.timeout_ms));
    let mut ok = false;
    let mut write_err = String::new();
    match resp6_res {
        Ok(resp) => match validate_modbus_response(&resp, 6) {
            Ok(()) => ok = true,
            Err(e) => write_err = format!("fc6 bad response: {e}"),
        },
        Err(e) => write_err = format!("fc6 send failed: {e}"),
    }
    if !ok {
        let mb16 = match modbus::sout_mb_only(conn.rtu, 16, binding.reg_mb, 1, Some(&dat)) {
            Ok(v) => v,
            Err(e) => {
                return CmdWorkerResult {
                    reg_id,
                    live_value: None,
                    last_cmd: "ERR".to_string(),
                    status: None,
                    err: Some(format!("fc16 build failed: {e}; prev={write_err}")),
                };
            }
        };
        let (_, resp16_res) = send_mb_over_udp(&conn, &mb16, Duration::from_millis(conn.timeout_ms));
        match resp16_res {
            Ok(resp) => match validate_modbus_response(&resp, 16) {
                Ok(()) => ok = true,
                Err(e) => write_err = format!("fc16 bad response: {e}; prev={write_err}"),
            },
            Err(e) => write_err = format!("fc16 send failed: {e}; prev={write_err}"),
        }
    }
    if ok {
        CmdWorkerResult {
            reg_id,
            live_value: Some(val),
            last_cmd: "OK WR".to_string(),
            status: Some(format!(
                "WRITE sent: reg {} mb={} value={:.3}",
                reg_id, binding.reg_mb, val
            )),
            err: None,
        }
    } else {
        CmdWorkerResult {
            reg_id,
            live_value: None,
            last_cmd: "ERR".to_string(),
            status: None,
            err: Some(format!("write failed for reg {}: {}", reg_id, write_err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_crc(mut frame: Vec<u8>) -> Vec<u8> {
        let crc = modbus::crc16(&frame);
        frame.push((crc & 0xFF) as u8);
        frame.push(((crc >> 8) & 0xFF) as u8);
        frame
    }

    fn wrap_udp(mb_frame: Vec<u8>) -> Vec<u8> {
        let mut resp = vec![0u8; 10];
        resp.extend_from_slice(&mb_frame);
        resp
    }

    #[test]
    fn validate_modbus_response_ok() {
        let resp = wrap_udp(with_crc(vec![1, 3, 2, 0x12, 0x34]));
        assert!(validate_modbus_response(&resp, 3).is_ok());
    }

    #[test]
    fn validate_modbus_response_exception() {
        let resp = wrap_udp(with_crc(vec![1, 0x83, 0x02]));
        let err = validate_modbus_response(&resp, 3).expect_err("exception response must fail");
        assert!(err.contains("modbus exception"));
    }

    #[test]
    fn validate_modbus_response_unexpected_func() {
        let resp = wrap_udp(with_crc(vec![1, 4, 2, 0x00, 0x01]));
        let err = validate_modbus_response(&resp, 3).expect_err("wrong function must fail");
        assert!(err.contains("unexpected func"));
    }

    #[test]
    fn parse_read_words_standard_frame() {
        let resp = wrap_udp(with_crc(vec![1, 3, 4, 0x12, 0x34, 0xAB, 0xCD]));
        let words = parse_read_words_from_resp(&resp, 3).expect("parse should succeed");
        assert_eq!(words, vec![0x1234, 0xABCD]);
    }

    #[test]
    fn parse_read_words_extended_unit_frame() {
        let resp = wrap_udp(with_crc(vec![0xF8, 0x01, 4, 0x00, 0x02, 0xBE, 0xEF]));
        let words = parse_read_words_from_resp(&resp, 4).expect("parse should succeed");
        assert_eq!(words, vec![0xBEEF]);
    }

    #[test]
    fn parse_read_words_rejects_odd_byte_count() {
        let resp = wrap_udp(with_crc(vec![1, 3, 3, 0x00, 0x01, 0x02]));
        let err = parse_read_words_from_resp(&resp, 3).expect_err("odd byte_count must fail");
        assert!(err.contains("bad byte_count"));
    }
}
