use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::app::{
    fmt_num_compact, hex_join, send_mb_over_udp, validate_modbus_response, Ss5App,
    IO_MAX_ROWS_PER_CLICK, IO_REQ_TIMEOUT_MS,
};
use crate::modbus;
use crate::modbus_service;
use crate::models::RegRow;
use crate::script::Script;
use eframe::egui;

impl Ss5App {
    pub(crate) fn open_kpz_io_window(&mut self) {
        self.kpz_io_open = true;
        let groups = self.graph_groups_for_selected_kpz();
        let current_ok = self
            .kpz_io_group_id
            .map(|id| groups.iter().any(|g| g.id == id))
            .unwrap_or(false);
        if !current_ok {
            self.kpz_io_group_id = groups.first().map(|g| g.id);
        }
        self.kpz_io_status = None;
        self.kpz_io_err = None;
        self.kpz_io_last_tx_hex.clear();
        self.kpz_io_last_rx_hex.clear();
        self.kpz_io_script_log.clear();
        self.reload_kpz_io_cmd5_rows();
        self.reload_kpz_io_input();
        self.reload_kpz_io_holding();
    }

    fn reload_kpz_io_cmd5_rows(&mut self) {
        self.kpz_io_cmd5_regs.clear();
        let Some(kpz_id) = self.selected_kpz else {
            self.kpz_io_cmd5_addr = None;
            return;
        };
        let Some(group_id) = self.kpz_io_group_id else {
            self.kpz_io_cmd5_addr = None;
            return;
        };
        match self.db.get_kpz_io_rows(kpz_id, group_id, 1) {
            Ok(mut rows) => {
                rows.sort_by_key(|r| r.mb);
                self.kpz_io_cmd5_regs = rows
                    .into_iter()
                    .map(|r| RegRow {
                        id: r.id,
                        name: r.name,
                        mb: r.mb,
                        tip: r.tip,
                        bits: r.bits,
                    })
                    .collect();
                let keep = self
                    .kpz_io_cmd5_addr
                    .map(|mb| self.kpz_io_cmd5_regs.iter().any(|r| r.mb == mb))
                    .unwrap_or(false);
                if !keep {
                    self.kpz_io_cmd5_addr = self.kpz_io_cmd5_regs.first().map(|r| r.mb);
                }
            }
            Err(e) => {
                self.kpz_io_err = Some(format!("read cmd5 regs failed: {e}"));
                self.kpz_io_cmd5_addr = None;
            }
        }
    }

    fn reload_kpz_io_input(&mut self) {
        self.kpz_io_err = None;
        let Some(kpz_id) = self.selected_kpz else {
            self.kpz_io_input_rows.clear();
            self.kpz_io_err = Some("KPZ не выбран".to_string());
            return;
        };
        let Some(group_id) = self.kpz_io_group_id else {
            self.kpz_io_input_rows.clear();
            return;
        };
        let Some(n_mb_id) = self.n_mb_id_by_name("TIT") else {
            self.kpz_io_input_rows.clear();
            self.kpz_io_err = Some("n_mb 'TIT' not found".to_string());
            return;
        };
        match self.db.get_kpz_io_rows(kpz_id, group_id, n_mb_id) {
            Ok(v) => {
                self.kpz_io_input_rows = v;
                let conn = match self.build_io_conn() {
                    Ok(c) => c,
                    Err(e) => {
                        self.kpz_io_err = Some(e);
                        return;
                    }
                };
                self.kpz_io_obj_info = format!(
                    "ip={} port={} rtu={} modem={} kanal={} speed={} stop={} parit={} bit={} max_pkt_len={}",
                    conn.ip, conn.port, conn.rtu, conn.modem, conn.kan, conn.speed, conn.stop, conn.par, conn.data, conn.max_pkt_len
                );
                let svc = Self::as_service_conn(&conn);
                let items: Vec<modbus_service::ReadItem> = self
                    .kpz_io_input_rows
                    .iter()
                    .take(IO_MAX_ROWS_PER_CLICK)
                    .map(|r| modbus_service::ReadItem {
                        id: r.id,
                        mb: r.mb,
                        tip: r.tip,
                        bits: r.bits,
                    })
                    .collect();
                match modbus_service::read_group_glued(
                    &svc,
                    4,
                    &items,
                    Duration::from_millis(IO_REQ_TIMEOUT_MS),
                ) {
                    Ok(res) => {
                        self.kpz_io_last_tx_hex = hex_join(&res.tx);
                        self.kpz_io_last_rx_hex = hex_join(&res.rx);
                        for row in self.kpz_io_input_rows.iter_mut() {
                            row.last_val = res.values_by_id.get(&row.id).copied();
                            if let Some(v) = row.last_val {
                                self.kpz_io_cached_vals.insert(row.id, v);
                                self.kpz_io_cached_vals.insert(row.mb, v);
                            }
                        }
                    }
                    Err(e) => {
                        for row in self.kpz_io_input_rows.iter_mut() {
                            row.last_val = None;
                        }
                        self.kpz_io_err = Some(format!("input read failed: {}", e));
                    }
                }
            }
            Err(e) => self.kpz_io_err = Some(format!("read input failed: {e}")),
        }
    }

    fn reload_kpz_io_holding(&mut self) {
        self.kpz_io_err = None;
        let Some(kpz_id) = self.selected_kpz else {
            self.kpz_io_holding_rows.clear();
            self.kpz_io_err = Some("KPZ не выбран".to_string());
            return;
        };
        let Some(group_id) = self.kpz_io_group_id else {
            self.kpz_io_holding_rows.clear();
            return;
        };
        let Some(n_mb_id) = self.n_mb_id_by_name("REG") else {
            self.kpz_io_holding_rows.clear();
            self.kpz_io_err = Some("n_mb 'REG' not found".to_string());
            return;
        };
        match self.db.get_kpz_io_rows(kpz_id, group_id, n_mb_id) {
            Ok(v) => {
                self.kpz_io_holding_rows = v;
                let conn = match self.build_io_conn() {
                    Ok(c) => c,
                    Err(e) => {
                        self.kpz_io_err = Some(e);
                        return;
                    }
                };
                self.kpz_io_obj_info = format!(
                    "ip={} port={} rtu={} modem={} kanal={} speed={} stop={} parit={} bit={} max_pkt_len={}",
                    conn.ip, conn.port, conn.rtu, conn.modem, conn.kan, conn.speed, conn.stop, conn.par, conn.data, conn.max_pkt_len
                );
                let svc = Self::as_service_conn(&conn);
                let items: Vec<modbus_service::ReadItem> = self
                    .kpz_io_holding_rows
                    .iter()
                    .take(IO_MAX_ROWS_PER_CLICK)
                    .map(|r| modbus_service::ReadItem {
                        id: r.id,
                        mb: r.mb,
                        tip: r.tip,
                        bits: r.bits,
                    })
                    .collect();
                match modbus_service::read_group_glued(
                    &svc,
                    3,
                    &items,
                    Duration::from_millis(IO_REQ_TIMEOUT_MS),
                ) {
                    Ok(res) => {
                        self.kpz_io_last_tx_hex = hex_join(&res.tx);
                        self.kpz_io_last_rx_hex = hex_join(&res.rx);
                        for row in self.kpz_io_holding_rows.iter_mut() {
                            row.reg_val = res.values_by_id.get(&row.id).copied();
                            if let Some(v) = row.reg_val {
                                self.kpz_io_cached_vals.insert(row.id, v);
                                self.kpz_io_cached_vals.insert(row.mb, v);
                            }
                        }
                    }
                    Err(e) => {
                        for row in self.kpz_io_holding_rows.iter_mut() {
                            row.reg_val = None;
                        }
                        self.kpz_io_err = Some(format!("holding read failed: {}", e));
                    }
                }
                if self.kpz_io_selected_holding.is_none() {
                    self.kpz_io_selected_holding = self.kpz_io_holding_rows.first().map(|r| r.id);
                } else if let Some(id) = self.kpz_io_selected_holding {
                    if !self.kpz_io_holding_rows.iter().any(|r| r.id == id) {
                        self.kpz_io_selected_holding = self.kpz_io_holding_rows.first().map(|r| r.id);
                    }
                }
                if let Some(id) = self.kpz_io_selected_holding {
                    if let Some(r) = self.kpz_io_holding_rows.iter().find(|x| x.id == id) {
                        self.kpz_io_write_value = r.reg_val.map(|v| v.to_string()).unwrap_or_default();
                    }
                }
            }
            Err(e) => self.kpz_io_err = Some(format!("read holding failed: {e}")),
        }
    }

    fn write_kpz_io_holding(&mut self) {
        self.kpz_io_err = None;
        self.kpz_io_status = None;
        let Some(reg_id) = self.kpz_io_selected_holding else {
            self.kpz_io_err = Some("No holding reg selected".to_string());
            return;
        };
        let val = match self.kpz_io_write_value.trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                self.kpz_io_err = Some("value must be number".to_string());
                return;
            }
        };
        let conn = match self.build_io_conn() {
            Ok(c) => c,
            Err(e) => {
                self.kpz_io_err = Some(e);
                return;
            }
        };
        let Some(row) = self.kpz_io_holding_rows.iter().find(|r| r.id == reg_id).cloned() else {
            self.kpz_io_err = Some(format!("holding reg {} not found", reg_id));
            return;
        };
        match Self::write_reg_direct(&conn, &row, val) {
            Ok((tx, rx)) => {
                self.kpz_io_last_tx_hex = hex_join(&tx);
                self.kpz_io_last_rx_hex = hex_join(&rx);
            }
            Err(e) => {
                self.kpz_io_err = Some(format!("write holding failed: {e}"));
                return;
            }
        }
        self.reload_kpz_io_holding();
        self.kpz_io_status = Some(format!("holding reg {} written directly", reg_id));
        self.push_log(format!("KPZ I/O: holding reg {} = {}", reg_id, val));
    }

    fn send_kpz_io_cmd5(&mut self, on: bool) {
        self.kpz_io_err = None;
        self.kpz_io_status = None;
        let Some(addr) = self.kpz_io_cmd5_addr else {
            self.kpz_io_err = Some("No mm/mb address selected for cmd5".to_string());
            return;
        };
        let conn = match self.build_io_conn() {
            Ok(c) => c,
            Err(e) => {
                self.kpz_io_err = Some(e);
                return;
            }
        };
        let dat = if on { [0x00u8, 0x01u8] } else { [0x00u8, 0x00u8] };
        let mb = match modbus::sout_mb_only(conn.rtu, 5, addr, 1, Some(&dat)) {
            Ok(v) => v,
            Err(e) => {
                self.kpz_io_err = Some(format!("cmd5 build failed: {e}"));
                return;
            }
        };
        let (tx, resp_res) = send_mb_over_udp(&conn, &mb, Duration::from_millis(IO_REQ_TIMEOUT_MS));
        match resp_res {
            Ok(rx) => {
                self.kpz_io_last_tx_hex = hex_join(&tx);
                self.kpz_io_last_rx_hex = hex_join(&rx);
                if let Err(e) = validate_modbus_response(&rx, 5) {
                    self.kpz_io_err = Some(format!("cmd5 bad response: {e}"));
                    return;
                }
                self.kpz_io_status = Some(format!(
                    "cmd5 sent: mb={} -> {}",
                    addr,
                    if on { "ON" } else { "OFF" }
                ));
                self.push_log(format!(
                    "KPZ I/O: cmd5 mb={} {}",
                    addr,
                    if on { "ON" } else { "OFF" }
                ));
            }
            Err(e) => {
                self.kpz_io_last_tx_hex = hex_join(&tx);
                self.kpz_io_err = Some(format!("cmd5 failed: {e}"));
            }
        }
    }

    fn run_kpz_io_script(&mut self) {
        self.kpz_io_err = None;
        self.kpz_io_status = None;
        self.kpz_io_script_log.clear();
        let Some(kpz_id) = self.selected_kpz else {
            self.kpz_io_err = Some("KPZ не выбран".to_string());
            return;
        };
        let Some(script_group) = self.kpz_io_group_id else {
            self.kpz_io_err = Some("No group selected".to_string());
            return;
        };
        let conn = match self.build_io_conn() {
            Ok(c) => c,
            Err(e) => {
                self.kpz_io_err = Some(e);
                return;
            }
        };
        let gs = match self.db.get_effective_g_script(script_group) {
            Ok(Some(v)) => v,
            Ok(None) => {
                self.kpz_io_err = Some(format!(
                    "effective g_script for group {} not found",
                    script_group
                ));
                return;
            }
            Err(e) => {
                self.kpz_io_err = Some(format!("get_effective_g_script failed: {e}"));
                return;
            }
        };
        if !gs.en {
            self.kpz_io_err = Some(format!("g_script group {} is disabled", script_group));
            return;
        }
        let pre_src = gs.pre_src.trim();
        let post_src = gs.post_src.trim();
        if pre_src.is_empty() {
            self.kpz_io_err = Some(format!("g_script group {} PRE is empty", script_group));
            return;
        }

        let mut rv: HashMap<i32, f64> = self.db.get_last_arx_vals(kpz_id).unwrap_or_default();
        for (k, v) in &self.kpz_io_cached_vals {
            rv.insert(*k, *v);
        }
        if !rv.contains_key(&70010) {
            if let Some(v) = rv.get(&400).copied() {
                rv.insert(70010, v);
            }
        }
        let mut svc_bases: HashSet<i32> = HashSet::new();
        let b0 = rv.get(&70010).copied().unwrap_or(0.0) as i32;
        if b0 > 0 {
            svc_bases.insert(b0);
        }
        let b400 = rv.get(&400).copied().unwrap_or(0.0) as i32;
        if b400 > 0 {
            svc_bases.insert(b400);
        }
        let reg_rows = self.db.get_all_reg_edit().unwrap_or_default();
        let mut reg_id_set: std::collections::HashSet<i32> = std::collections::HashSet::new();
        let mut reg_by_mb: HashMap<i32, i32> = HashMap::new();
        let nmb_tit = self.n_mb_id_by_name("TIT");
        let nmb_reg = self.n_mb_id_by_name("REG");
        let mut func_by_mb: HashMap<i32, u8> = HashMap::new();
        let mut _group_tit = 0usize;
        let mut _group_reg = 0usize;
        for r in &reg_rows {
            reg_id_set.insert(r.id);
            reg_by_mb.entry(r.mb).or_insert(r.id);
            if r.grup == Some(script_group) {
                if Some(r.n_mb.unwrap_or_default()) == nmb_tit {
                    _group_tit += 1;
                } else if Some(r.n_mb.unwrap_or_default()) == nmb_reg {
                    _group_reg += 1;
                }
            }
            if Some(r.n_mb.unwrap_or_default()) == nmb_tit {
                func_by_mb.insert(r.mb, 4);
            } else if Some(r.n_mb.unwrap_or_default()) == nmb_reg {
                func_by_mb.insert(r.mb, 3);
            }
        }
        let _ = func_by_mb;
        let print_buf = RefCell::new(String::new());

        let pre = match Script::parse(pre_src) {
            Ok(v) => v,
            Err(e) => {
                self.kpz_io_err = Some(format!("PRE parse failed: {e}"));
                return;
            }
        };
        let pre_out = match pre.eval_result(
            &[],
            true,
            &|rid| rv.get(&rid).copied().unwrap_or(0.0),
            &|_, _| 0.0,
            Some(&|msg| {
                let mut b = print_buf.borrow_mut();
                if !b.is_empty() {
                    b.push('\n');
                }
                b.push_str("[PRE] ");
                b.push_str(msg);
            }),
            None,
            100000,
        ) {
            Ok(v) => v,
            Err(e) => {
                self.kpz_io_err = Some(format!("PRE eval failed: {e}"));
                return;
            }
        };
        for (k, v) in &pre_out.regs {
            rv.insert(*k, *v);
        }
        let b70010_after_pre = rv.get(&70010).copied().unwrap_or(0.0) as i32;
        if b70010_after_pre > 0 {
            svc_bases.insert(b70010_after_pre);
        }
        let b400_after_pre = rv.get(&400).copied().unwrap_or(0.0) as i32;
        if b400_after_pre > 0 {
            svc_bases.insert(b400_after_pre);
        }
        let cmds = crate::app::decode_pre_cmds(
            &pre_out.regs,
            gs.max_k.clamp(1, 16),
            gs.max_words.clamp(1, 2500),
        );
        if let Some(b1) = pre_out.regs.get(&1).copied().map(|v| v as i32) {
            if b1 > 0 {
                svc_bases.insert(b1);
            }
        }
        let b2 = rv.get(&1).copied().unwrap_or(0.0) as i32;
        if b2 > 0 {
            svc_bases.insert(b2);
        }
        if cmds.is_empty() {
            self.kpz_io_err = Some("PRE produced no commands".to_string());
            return;
        }

        let post = if post_src.is_empty() {
            None
        } else {
            match Script::parse(post_src) {
                Ok(v) => Some(v),
                Err(e) => {
                    self.kpz_io_err = Some(format!("POST parse failed: {e}"));
                    return;
                }
            }
        };

        let mut written = 0usize;
        let mut script_tx_hex: Option<String> = None;
        let mut script_rx_hex: Option<String> = None;
        for cmd in &cmds {
            let func: u8 = 4;
            let mb = match modbus::sout_mb_only(conn.rtu, func, cmd.addr_human, cmd.cnt_words as u16, None) {
                Ok(v) => v,
                Err(e) => {
                    self.kpz_io_err = Some(format!("build cmd failed: {e}"));
                    return;
                }
            };
            let (tx, resp_res) =
                send_mb_over_udp(&conn, &mb, Duration::from_millis(IO_REQ_TIMEOUT_MS.max(1200)));
            let resp = match resp_res {
                Ok(v) => v,
                Err(e) => {
                    self.kpz_io_last_tx_hex = hex_join(&tx);
                    self.kpz_io_err = Some(format!("script cmd send failed: {e}"));
                    return;
                }
            };
            self.kpz_io_last_tx_hex = hex_join(&tx);
            self.kpz_io_last_rx_hex = hex_join(&resp);
            script_tx_hex = Some(self.kpz_io_last_tx_hex.clone());
            script_rx_hex = Some(self.kpz_io_last_rx_hex.clone());
            let words = match crate::app::parse_read_words_from_resp(&resp, func) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if words.len() < cmd.cnt_words as usize {
                continue;
            }
            let words = words[..cmd.cnt_words as usize].to_vec();
            rv.insert(20, words.len() as f64);
            rv.insert(21, cmd.cnt_words as f64);
            for base in &svc_bases {
                rv.insert(*base + 20, words.len() as f64);
                rv.insert(*base + 21, cmd.cnt_words as f64);
            }
            if let Some(post_script) = &post {
                let post_out = match post_script.eval_result(
                    &words,
                    false,
                    &|rid| rv.get(&rid).copied().unwrap_or(0.0),
                    &|_, _| 0.0,
                    Some(&|msg| {
                        let mut b = print_buf.borrow_mut();
                        if !b.is_empty() {
                            b.push('\n');
                        }
                        b.push_str("[POST] ");
                        b.push_str(msg);
                    }),
                    None,
                    100000,
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        self.kpz_io_err = Some(format!("POST eval failed: {e}"));
                        return;
                    }
                };
                {
                    let mut b = print_buf.borrow_mut();
                    if !b.is_empty() {
                        b.push('\n');
                    }
                    b.push_str(&format!(
                        "[POST] result regs={} emits={}",
                        post_out.regs.len(),
                        post_out.emits.len()
                    ));
                }
                for (reg_id, val) in &post_out.regs {
                    let target_id: Option<i32> =
                        if self.db.update_reg_val_checked(*reg_id, *val).unwrap_or(0) > 0 {
                            Some(*reg_id)
                        } else if reg_id_set.contains(reg_id) {
                            Some(*reg_id)
                        } else {
                            reg_by_mb.get(reg_id).copied()
                        };
                    if let Some(target_id) = target_id {
                        if self.db.update_reg_val(target_id, *val).is_ok() {
                            rv.insert(*reg_id, *val);
                            rv.insert(target_id, *val);
                            if let Some(mb) =
                                reg_rows.iter().find(|r| r.id == target_id).map(|r| r.mb)
                            {
                                rv.insert(mb, *val);
                                self.kpz_io_cached_vals.insert(mb, *val);
                            }
                            self.kpz_io_cached_vals.insert(target_id, *val);
                            self.kpz_io_cached_vals.insert(*reg_id, *val);
                            written += 1;
                        }
                    } else {
                        let mut b = print_buf.borrow_mut();
                        if !b.is_empty() {
                            b.push('\n');
                        }
                        b.push_str(&format!("[POST] skip key {} (no reg.id/mb match)", reg_id));
                    }
                }
                for ev in &post_out.emits {
                    let reg_id = ev.reg_id;
                    let val = ev.value;
                    {
                        let mut b = print_buf.borrow_mut();
                        if !b.is_empty() {
                            b.push('\n');
                        }
                        b.push_str(&format!("[POST] emit {}={}", reg_id, val));
                    }
                    let target_id: Option<i32> =
                        if self.db.update_reg_val_checked(reg_id, val).unwrap_or(0) > 0 {
                            Some(reg_id)
                        } else if reg_id_set.contains(&reg_id) {
                            Some(reg_id)
                        } else {
                            reg_by_mb.get(&reg_id).copied()
                        };
                    if let Some(target_id) = target_id {
                        if self.db.update_reg_val(target_id, val).is_ok() {
                            rv.insert(reg_id, val);
                            rv.insert(target_id, val);
                            if let Some(mb) =
                                reg_rows.iter().find(|r| r.id == target_id).map(|r| r.mb)
                            {
                                rv.insert(mb, val);
                                self.kpz_io_cached_vals.insert(mb, val);
                            }
                            self.kpz_io_cached_vals.insert(target_id, val);
                            self.kpz_io_cached_vals.insert(reg_id, val);
                            written += 1;
                        }
                    } else {
                        let mut b = print_buf.borrow_mut();
                        if !b.is_empty() {
                            b.push('\n');
                        }
                        b.push_str(&format!("[POST] skip emit {} (no reg.id/mb match)", reg_id));
                    }
                }
            }
        }

        self.reload_kpz_io_input();
        self.reload_kpz_io_holding();
        if let Some(tx) = script_tx_hex {
            self.kpz_io_last_tx_hex = tx;
        }
        if let Some(rx) = script_rx_hex {
            self.kpz_io_last_rx_hex = rx;
        }
        self.kpz_io_script_log = print_buf.into_inner();
        self.kpz_io_status = Some(format!(
            "Скрипт выполнен: группа {}, команд={}, записано={}",
            script_group,
            cmds.len(),
            written
        ));
    }

    pub(crate) fn show_kpz_io_window(&mut self, ctx: &egui::Context) {
        if !self.kpz_io_open {
            return;
        }

        let mut open = self.kpz_io_open;
        egui::Window::new("Ввод/вывод KPZ")
            .open(&mut open)
            .resizable(true)
            .default_size([1080.0, 680.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("KPZ: {}", self.selected_kpz_name()));
                    ui.separator();
                    ui.label("Группа:");
                    let groups = self.graph_groups_for_selected_kpz();
                    let mut group_id = self.kpz_io_group_id;
                    let selected_text = group_id
                        .and_then(|id| groups.iter().find(|g| g.id == id))
                        .map(|g| {
                            if g.name.is_empty() {
                                g.id.to_string()
                            } else {
                                format!("{} - {}", g.id, g.name)
                            }
                        })
                        .unwrap_or_else(|| "<нет>".to_string());
                    egui::ComboBox::from_id_salt("kpz_io_group")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            for g in &groups {
                                let label = if g.name.is_empty() {
                                    g.id.to_string()
                                } else {
                                    format!("{} - {}", g.id, g.name)
                                };
                                ui.selectable_value(&mut group_id, Some(g.id), label);
                            }
                        });
                    if group_id != self.kpz_io_group_id {
                        self.kpz_io_group_id = group_id;
                        self.reload_kpz_io_cmd5_rows();
                        self.reload_kpz_io_input();
                        self.reload_kpz_io_holding();
                    }
                    if ui.button("Читать input").clicked() {
                        self.reload_kpz_io_input();
                    }
                    if ui.button("Читать holding").clicked() {
                        self.reload_kpz_io_holding();
                    }
                    if ui.button("Script").clicked() {
                        self.run_kpz_io_script();
                    }
                });

                if let Some(err) = &self.kpz_io_err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &self.kpz_io_status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }
                if !self.kpz_io_obj_info.is_empty() {
                    ui.label(format!("Параметры OBJ: {}", self.kpz_io_obj_info));
                }
                ui.separator();
                ui.label("TX hex:");
                ui.add(
                    egui::TextEdit::multiline(&mut self.kpz_io_last_tx_hex)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                ui.label("RX hex:");
                ui.add(
                    egui::TextEdit::multiline(&mut self.kpz_io_last_rx_hex)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                ui.label("Лог скрипта:");
                ui.add(
                    egui::TextEdit::multiline(&mut self.kpz_io_script_log)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY),
                );

                ui.separator();
                ui.columns(2, |cols| {
                    let tu_mode = !self.kpz_io_cmd5_regs.is_empty();
                    cols[0].heading("Input (n_mb = TIT)");
                    egui::ScrollArea::vertical()
                        .id_salt("kpz_io_input_list")
                        .max_height(520.0)
                        .show(&mut cols[0], |ui| {
                            for r in &self.kpz_io_input_rows {
                                let val = r
                                    .last_val
                                    .or(r.reg_val)
                                    .map(fmt_num_compact)
                                    .unwrap_or_else(|| "-".to_string());
                                ui.label(format!(
                                    "{}  {}  mb={} tip={} bits={} val={}",
                                    r.id,
                                    r.name,
                                    r.mb,
                                    r.tip,
                                    r.bits
                                        .map(|v| v.to_string())
                                        .unwrap_or_else(|| "-".to_string()),
                                    val
                                ));
                            }
                        });

                    if !tu_mode {
                        cols[1].heading("Holding (n_mb = REG)");
                        egui::ScrollArea::vertical()
                            .id_salt("kpz_io_holding_list")
                            .max_height(420.0)
                            .show(&mut cols[1], |ui| {
                                for r in &self.kpz_io_holding_rows {
                                    let val = r
                                        .reg_val
                                        .map(fmt_num_compact)
                                        .unwrap_or_else(|| "-".to_string());
                                    let label = format!(
                                        "{}  {}  mb={} tip={} bits={} val={}",
                                        r.id,
                                        r.name,
                                        r.mb,
                                        r.tip,
                                        r.bits
                                            .map(|v| v.to_string())
                                            .unwrap_or_else(|| "-".to_string()),
                                        val
                                    );
                                    if ui
                                        .selectable_label(self.kpz_io_selected_holding == Some(r.id), label)
                                        .clicked()
                                    {
                                        self.kpz_io_selected_holding = Some(r.id);
                                        self.kpz_io_write_value =
                                            r.reg_val.map(|v| v.to_string()).unwrap_or_default();
                                    }
                                }
                            });
                    }
                    if !tu_mode {
                        cols[1].separator();
                        cols[1].horizontal(|ui| {
                            ui.label("Записать значение:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.kpz_io_write_value)
                                    .desired_width(120.0),
                            );
                            if ui.button("Записать holding").clicked() {
                                self.write_kpz_io_holding();
                            }
                        });
                    }
                    cols[1].separator();
                    cols[1].horizontal(|ui| {
                        ui.label("Cmd5 mm/mb:");
                        let mut addr = self.kpz_io_cmd5_addr;
                        let selected = addr
                            .and_then(|mb| {
                                self.kpz_io_cmd5_regs
                                    .iter()
                                    .find(|r| r.mb == mb)
                                    .map(|r| format!("{} ({})", r.mb, r.name))
                            })
                            .unwrap_or_else(|| "<нет>".to_string());
                        egui::ComboBox::from_id_salt("kpz_io_cmd5_addr")
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                for r in &self.kpz_io_cmd5_regs {
                                    ui.selectable_value(
                                        &mut addr,
                                        Some(r.mb),
                                        format!("{} ({})", r.mb, r.name),
                                    );
                                }
                            });
                        self.kpz_io_cmd5_addr = addr;
                        if ui.button("ON").clicked() {
                            self.send_kpz_io_cmd5(true);
                        }
                        if ui.button("OFF").clicked() {
                            self.send_kpz_io_cmd5(false);
                        }
                    });
                });
            });
        self.kpz_io_open = open;
    }
}
