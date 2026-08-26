use std::collections::{BTreeSet, HashMap};
use std::net::{IpAddr, UdpSocket};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{Datelike, Local, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use eframe::egui;

use crate::db::Db;
use crate::modbus;
use crate::modbus_service;
use crate::models::{
    AlarmEventRow, AlarmRuleRow, AlarmStateRow, ArxSeriesRow, ArxStateRow, ArxValViewRow, DictItemRow, ElamRow,
    GScriptTemplateRow, GroupRow, KpzIoRow, KpzRow, ObjRow, PollLogRow, RegEditRow, RegRow,
    UiWindowGroupRow,
};
use crate::ui::window_link_editor::{show_ui_link_editor, UiLinkEditorAction, UiLinkEditorState};
use crate::utils::{decode_groups, hex_full};

mod windows;

pub struct Ss5App {
    db: Db,
    kpz: Vec<KpzRow>,
    groups: Vec<GroupRow>,
    obj_rows: Vec<ObjRow>,
    ref_ip: HashMap<i32, String>,
    ref_port: HashMap<i32, String>,
    ref_speed: HashMap<i32, String>,
    ref_parit: HashMap<i32, String>,
    ref_bit: HashMap<i32, String>,
    ref_stop: HashMap<i32, String>,
    ref_kanal: HashMap<i32, String>,
    ref_n_mb: HashMap<i32, String>,
    ref_tip: HashMap<i32, String>,
    ref_c: HashMap<i32, String>,
    selected_kpz: Option<i32>,
    kpz_filter: String,
    show_all_kpz: bool,
    start_flag: bool,
    poll_log: Vec<PollLogRow>,
    elam: Vec<ElamRow>,
    poll_log_limit: i64,
    elam_limit: i64,
    elam_ts_from: String,
    elam_ts_to: String,
    elam_time_picker_open: bool,
    elam_time_picker_target: ElamTimeTarget,
    elam_time_picker_year: i32,
    elam_time_picker_month: u32,
    elam_time_picker_day: u32,
    elam_time_picker_hour: u32,
    elam_time_picker_min: u32,
    elam_time_picker_sec: u32,
    load_err: Option<String>,
    kpz_save_status: Option<String>,
    auto_refresh_every: Duration,
    last_auto_refresh: Instant,
    kpz_t_a_edit: String,
    kpz_t_script_edit: String,
    selected_elam_index: Option<usize>,
    show_elam_details: bool,
    elam_full_hex: bool,
    group_editor_open: bool,
    group_edit_selected: BTreeSet<i32>,
    group_edit_filter: String,
    group_edit_err: Option<String>,
    group_edit_dirty: bool,
    gscript_open: bool,
    gscript_output_open: bool,
    gscript_group_id: Option<i32>,
    gscript_group_ids_db: Vec<i32>,
    gscript_templates: Vec<GScriptTemplateRow>,
    gscript_template_id: Option<i64>,
    gscript_template_name: String,
    gscript_pre: String,
    gscript_post: String,
    gscript_elam: bool,
    gscript_max_words: i32,
    gscript_max_k: i32,
    gscript_en: bool,
    gscript_ver: i32,
    gscript_status: Option<String>,
    gscript_err: Option<String>,
    gscript_dirty: bool,
    gscript_words_json: String,
    gscript_regs_json: String,
    gscript_hi_lo: bool,
    gscript_print_log: String,
    gscript_regs_out: Vec<(i32, f64)>,
    gscript_emits_out: Vec<(f64, i32, f64)>,
    gscript_use_rv_fallback: bool,
    gscript_tab: GScriptTab,
    gscript_output_tab: GScriptOutputTab,
    gscript_help_open: bool,
    graph_open: bool,
    graph_group_id: Option<i32>,
    graph_regs: Vec<RegRow>,
    graph_selected_regs: BTreeSet<i32>,
    graph_series: Vec<ArxSeriesRow>,
    graph_window_sec: i64,
    graph_limit: i64,
    graph_err: Option<String>,
    kpz_io_open: bool,
    kpz_io_group_id: Option<i32>,
    kpz_io_input_rows: Vec<KpzIoRow>,
    kpz_io_holding_rows: Vec<KpzIoRow>,
    kpz_io_selected_holding: Option<i32>,
    kpz_io_write_value: String,
    kpz_io_cmd5_regs: Vec<RegRow>,
    kpz_io_cmd5_addr: Option<i32>,
    kpz_io_status: Option<String>,
    kpz_io_err: Option<String>,
    kpz_io_obj_info: String,
    kpz_io_last_tx_hex: String,
    kpz_io_last_rx_hex: String,
    kpz_io_script_log: String,
    kpz_io_cached_vals: HashMap<i32, f64>,
    kpz_editor_open: bool,
    kpz_editor_id: String,
    kpz_editor_name: String,
    kpz_editor_rtu: String,
    kpz_editor_obj: Option<i32>,
    kpz_editor_modem: String,
    kpz_editor_max_pkt_len: String,
    kpz_editor_start: bool,
    kpz_editor_en_post: bool,
    kpz_editor_t_a: String,
    kpz_editor_t_script: String,
    kpz_editor_status: Option<String>,
    kpz_editor_err: Option<String>,
    obj_editor_open: bool,
    obj_editor_selected_id: Option<i32>,
    obj_editor_name: String,
    obj_editor_ip: Option<i32>,
    obj_editor_port: Option<i32>,
    obj_editor_kanal: Option<i32>,
    obj_editor_speed: Option<i32>,
    obj_editor_stop: Option<i32>,
    obj_editor_parit: Option<i32>,
    obj_editor_bit: Option<i32>,
    obj_editor_status: Option<String>,
    obj_editor_err: Option<String>,
    reg_editor_open: bool,
    reg_rows: Vec<RegEditRow>,
    reg_editor_selected_id: Option<i32>,
    reg_editor_id: String,
    reg_editor_group_filter: Option<i32>,
    reg_editor_filter: String,
    reg_editor_name: String,
    reg_editor_mb: String,
    reg_editor_n_mb: String,
    reg_editor_tip: String,
    reg_editor_bits: String,
    reg_editor_grup: String,
    reg_editor_a_en: bool,
    reg_editor_a_no_write: String,
    reg_editor_status: Option<String>,
    reg_editor_err: Option<String>,
    dict_editor_open: bool,
    dict_table: String,
    dict_items: Vec<DictItemRow>,
    dict_editor_selected_id: Option<i32>,
    dict_editor_id: String,
    dict_editor_name: String,
    dict_editor_status: Option<String>,
    dict_editor_err: Option<String>,
    alarm_open: bool,
    alarm_rules: Vec<AlarmRuleRow>,
    alarm_state_rows: Vec<AlarmStateRow>,
    alarm_events: Vec<AlarmEventRow>,
    alarm_selected_rule_id: Option<i64>,
    alarm_rule_kpz_id: String,
    alarm_group_id: Option<i32>,
    alarm_group_regs: Vec<RegRow>,
    alarm_rule_reg_id: String,
    alarm_rule_enabled: bool,
    alarm_rule_cmp: String,
    alarm_rule_set_lo: String,
    alarm_rule_set_hi: String,
    alarm_rule_set_lo_1: String,
    alarm_rule_set_hi_1: String,
    alarm_rule_hysteresis: String,
    alarm_rule_on_delay: String,
    alarm_rule_off_delay: String,
    alarm_rule_severity: String,
    alarm_rule_code: String,
    alarm_rule_message: String,
    alarm_rule_chat_id: String,
    alarm_tg_on_on: bool,
    alarm_tg_on_off: bool,
    alarm_tg_thr_main: bool,
    alarm_tg_thr_lvl1: bool,
    alarm_events_limit: i64,
    alarm_state_limit: i64,
    alarm_status: Option<String>,
    alarm_err: Option<String>,
    range_kpz_open: bool,
    range_kpz_id_start: String,
    range_kpz_id_end: String,
    range_kpz_obj: Option<i32>,
    range_kpz_modem_start: String,
    range_kpz_t_a: String,
    range_kpz_t_script: String,
    range_kpz_groups_selected: BTreeSet<i32>,
    range_kpz_start_enabled: bool,
    range_kpz_help_open: bool,
    range_kpz_status: Option<String>,
    range_kpz_err: Option<String>,
    runtime_cfg_open: bool,
    runtime_no_resp_failures: String,
    runtime_no_resp_backoff_sec: String,
    runtime_metrics_p95_warn_ms: String,
    runtime_metrics_p95_crit_ms: String,
    runtime_modbus_a_timeout_ms: String,
    runtime_modbus_script_timeout_ms: String,
    runtime_cfg_status: Option<String>,
    runtime_cfg_err: Option<String>,
    arx_state_open: bool,
    arx_state_rows: Vec<ArxStateRow>,
    arx_state_limit: i64,
    arx_state_kpz_filter: String,
    arx_state_kpz_id: String,
    arx_state_arx_id: String,
    arx_state_last_ind: String,
    arx_state_status: Option<String>,
    arx_state_err: Option<String>,
    arx_val_open: bool,
    arx_val_rows: Vec<ArxValViewRow>,
    arx_val_limit: i64,
    arx_val_kpz_filter: String,
    arx_val_status: Option<String>,
    arx_val_err: Option<String>,
    ui_link_editor: UiLinkEditorState,
    ui_log: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GScriptTab {
    Pre,
    Post,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GScriptOutputTab {
    Print,
    Regs,
    Emits,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ElamTimeTarget {
    From,
    To,
}

/// Function: $name.
fn is_script_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

/// Function: $name.
fn is_script_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

/// Function: $name.
fn push_job_text(
    job: &mut egui::text::LayoutJob,
    text: &str,
    color: egui::Color32,
    font_size: f32,
) {
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::monospace(font_size),
            color,
            ..Default::default()
        },
    );
}

/// Function: $name.
fn gscript_layout_job(ui: &egui::Ui, src: &str, wrap_width: f32) -> egui::text::LayoutJob {
    let base = ui.visuals().text_color();
    let kw = egui::Color32::from_rgb(120, 225, 255);
    let func = egui::Color32::from_rgb(78, 226, 180);
    let num = egui::Color32::from_rgb(255, 206, 112);
    let comment = egui::Color32::from_rgb(133, 158, 133);
    let string = egui::Color32::from_rgb(255, 164, 139);
    let op = egui::Color32::from_rgb(168, 193, 244);
    let brace_palette = [
        egui::Color32::from_rgb(255, 200, 120),
        egui::Color32::from_rgb(120, 220, 255),
        egui::Color32::from_rgb(170, 240, 170),
        egui::Color32::from_rgb(255, 170, 220),
    ];
    let brace_unmatched = egui::Color32::from_rgb(255, 96, 96);
    let font_size = 13.5;

    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;
    let mut brace_depth: usize = 0;

    let mut i = 0usize;
    while i < src.len() {
        let mut it = src[i..].chars();
        let Some(c) = it.next() else { break };
        let c_len = c.len_utf8();
        let next_ch = it.next();
        let rest = &src[i..];
        let advance_one = |idx: &mut usize| {
            *idx += c_len;
        };

        if c == '#' {
            let start = i;
            while i < src.len() {
                let Some(ch) = src[i..].chars().next() else { break };
                if ch == '\n' {
                    break;
                }
                i += ch.len_utf8();
            }
            push_job_text(&mut job, &src[start..i], comment, font_size);
            continue;
        }

        if rest.starts_with("//") {
            let start = i;
            i += 2;
            while i < src.len() {
                let Some(ch) = src[i..].chars().next() else { break };
                if ch == '\n' {
                    break;
                }
                i += ch.len_utf8();
            }
            push_job_text(&mut job, &src[start..i], comment, font_size);
            continue;
        }

        if c == '"' {
            let start = i;
            advance_one(&mut i);
            while i < src.len() {
                let Some(ch) = src[i..].chars().next() else { break };
                i += ch.len_utf8();
                if ch == '\\' && i < src.len() {
                    if let Some(esc) = src[i..].chars().next() {
                        i += esc.len_utf8();
                    }
                    continue;
                }
                if ch == '"' {
                    break;
                }
            }
            push_job_text(&mut job, &src[start..i], string, font_size);
            continue;
        }

        if c.is_ascii_digit() || (c == '.' && next_ch.map(|x| x.is_ascii_digit()).unwrap_or(false))
        {
            let start = i;
            if c == '.' {
                advance_one(&mut i);
            }
            while i < src.len() {
                let Some(ch) = src[i..].chars().next() else { break };
                if !ch.is_ascii_digit() {
                    break;
                }
                i += ch.len_utf8();
            }
            if i < src.len() && src[i..].starts_with('.') {
                i += '.'.len_utf8();
                while i < src.len() {
                    let Some(ch) = src[i..].chars().next() else { break };
                    if !ch.is_ascii_digit() {
                        break;
                    }
                    i += ch.len_utf8();
                }
            }
            if i < src.len() {
                let Some(ch) = src[i..].chars().next() else {
                    push_job_text(&mut job, &src[start..i], num, font_size);
                    continue;
                };
                if ch == 'e' || ch == 'E' {
                    let exp_pos = i;
                    i += ch.len_utf8();
                    if i < src.len() {
                        let Some(sign) = src[i..].chars().next() else {
                            push_job_text(&mut job, &src[start..i], num, font_size);
                            continue;
                        };
                        if sign == '+' || sign == '-' {
                            i += sign.len_utf8();
                        }
                    }
                    let digits_start = i;
                    while i < src.len() {
                        let Some(d) = src[i..].chars().next() else { break };
                        if !d.is_ascii_digit() {
                            break;
                        }
                        i += d.len_utf8();
                    }
                    if digits_start == i {
                        i = exp_pos;
                    }
                }
            }
            push_job_text(&mut job, &src[start..i], num, font_size);
            continue;
        }

        if is_script_ident_start(c) {
            let start = i;
            advance_one(&mut i);
            while i < src.len() {
                let Some(ch) = src[i..].chars().next() else { break };
                if !is_script_ident_continue(ch) {
                    break;
                }
                i += ch.len_utf8();
            }
            let tok = &src[start..i];
            let color = match tok {
                "if" | "else" | "while" | "for" | "let" | "reg" | "emit" => kw,
                "u16" | "i16" | "u32" | "i32" | "f32" | "dt2unix" | "rv" | "print"
                | "abs" | "sqrt" | "floor" | "ceil" | "round" | "bit" | "av" | "print2"
                | "min" | "max" | "pow" | "clamp" => func,
                _ => base,
            };
            push_job_text(&mut job, tok, color, font_size);
            continue;
        }

        let two_ops = ["<<", ">>", "<=", ">=", "==", "!=", "&&", "||"];
        if let Some(two) = two_ops.iter().find(|t| rest.starts_with(**t)) {
            push_job_text(&mut job, two, op, font_size);
            i += two.len();
            continue;
        }

        if "(){};=+-*/%,&|^<>!,".contains(c) {
            let end = i + c_len;
            let color = if c == '{' {
                let clr = brace_palette[brace_depth % brace_palette.len()];
                brace_depth = brace_depth.saturating_add(1);
                clr
            } else if c == '}' {
                if brace_depth == 0 {
                    brace_unmatched
                } else {
                    brace_depth -= 1;
                    brace_palette[brace_depth % brace_palette.len()]
                }
            } else {
                op
            };
            push_job_text(&mut job, &src[i..end], color, font_size);
            i = end;
            continue;
        }

        let end = i + c_len;
        push_job_text(&mut job, &src[i..end], base, font_size);
        i = end;
    }

    job
}

/// Function: $name.
fn fmt_unix_ts(sec_f64: f64, with_newline: bool) -> String {
    if !sec_f64.is_finite() {
        return "-".to_string();
    }
    let sec = sec_f64.round() as i64;
    // Protect chrono conversion from unrealistic axis values during aggressive zoom/scroll.
    if !(-2_208_988_800..=4_102_444_800).contains(&sec) {
        return format!("{:.0}", sec_f64);
    }
    match Local.timestamp_opt(sec, 0) {
        LocalResult::Single(dt) => {
            if with_newline {
                dt.format("%Y-%m-%d\n%H:%M:%S").to_string()
            } else {
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            }
        }
        _ => format!("{:.0}", sec_f64),
    }
}

/// Function: $name.
fn parse_time_filter_to_unix_sec(input: &str) -> Result<Option<i64>, String> {
    let s = input.trim();
    if s.is_empty() {
        return Ok(None);
    }
    if let Ok(v) = s.parse::<i64>() {
        return Ok(Some(v));
    }

    let fmts = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%d.%m.%Y %H:%M:%S",
        "%d.%m.%Y %H:%M",
    ];
    for fmt in fmts {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, fmt) {
            return match Local.from_local_datetime(&naive) {
                LocalResult::Single(dt) => Ok(Some(dt.timestamp())),
                LocalResult::Ambiguous(dt, _) => Ok(Some(dt.timestamp())),
                LocalResult::None => Err("Р’СЂРµРјСЏ РІРЅРµ Р»РѕРєР°Р»СЊРЅРѕР№ Р·РѕРЅС‹".to_string()),
            };
        }
    }

    Err("РќРµРІРµСЂРЅС‹Р№ С„РѕСЂРјР°С‚ РІСЂРµРјРµРЅРё. РСЃРїРѕР»СЊР·СѓР№С‚Рµ YYYY-MM-DD HH:MM[:SS], DD.MM.YYYY HH:MM[:SS] РёР»Рё Unix".to_string())
}

/// РР·РІР»РµРєР°РµС‚ РєРѕР»РёС‡РµСЃС‚РІРѕ РїСЂРёРЅСЏС‚С‹С… СЃР»РѕРІ РёР· Modbus-РѕС‚РІРµС‚Р° (`resp`) РґР»СЏ ELAM-РѕС‚РѕР±СЂР°Р¶РµРЅРёСЏ.
///
/// # Parameters
/// - `resp`: СЃС‹СЂРѕР№ UDP/Modbus РїР°РєРµС‚ РѕС‚РІРµС‚Р°.
///
/// # Returns
/// - `Some(words)`, РµСЃР»Рё РїР°РєРµС‚ РїРѕС…РѕР¶ РЅР° РІР°Р»РёРґРЅС‹Р№ read-response Рё `byte_count` РєРѕСЂСЂРµРєС‚РµРЅ.
/// - `None`, РµСЃР»Рё РґРµРєРѕРґРёСЂРѕРІР°РЅРёРµ РЅРµРІРѕР·РјРѕР¶РЅРѕ.
///
/// # РџСЂРёРјРµСЂ
/// - `let received = rx_words_from_resp_packet(resp_bytes);`
fn rx_words_from_resp_packet(resp: &[u8]) -> Option<usize> {
    const HDR: usize = 10;
    let mb = if resp.len() > HDR + 3 { &resp[HDR..] } else { resp };
    if mb.is_empty() {
        return None;
    }
    let ulen = if mb[0] >= 0xF8 { 2 } else { 1 };
    let func_index = ulen;
    if mb.len() <= func_index {
        return None;
    }
    let func = mb[func_index];
    if (func & 0x80) != 0 {
        return None;
    }
    let bc_index = func_index + 1;
    let byte_count = if ulen == 2 {
        if mb.len() < bc_index + 2 {
            return None;
        }
        ((mb[bc_index] as usize) << 8) | (mb[bc_index + 1] as usize)
    } else {
        if mb.len() <= bc_index {
            return None;
        }
        mb[bc_index] as usize
    };
    if byte_count == 0 || (byte_count % 2) != 0 {
        return None;
    }
    Some(byte_count / 2)
}

/// РћРїСЂРµРґРµР»СЏРµС‚, СЏРІР»СЏРµС‚СЃСЏ Р»Рё СЃС‚СЂРѕРєР° ELAM Р°РіСЂРµРіРёСЂРѕРІР°РЅРЅРѕР№ `SUMMARY`-Р·Р°РїРёСЃСЊСЋ.
///
/// # Parameters
/// - `row`: СЃС‚СЂРѕРєР° ELAM РёР· Р‘Р”.
///
/// # Returns
/// - `true`, РµСЃР»Рё СЌС‚Рѕ summary (`func=0` РёР»Рё `status` РЅР°С‡РёРЅР°РµС‚СЃСЏ СЃ `SUMMARY:`).
/// - `false` РґР»СЏ РѕР±С‹С‡РЅРѕР№ per-request СЃС‚СЂРѕРєРё.
fn elam_is_summary(row: &ElamRow) -> bool {
    row.func == Some(0) || row.status.starts_with("SUMMARY:")
}

/// РќРѕСЂРјР°Р»РёР·СѓРµС‚ РїР°СЂСѓ `РѕР¶РёРґР°Р»РѕСЃСЊ/РїРѕР»СѓС‡РµРЅРѕ` РґР»СЏ СЃС‚СЂРѕРєРё ELAM.
///
/// # Parameters
/// - `row`: СЃС‚СЂРѕРєР° ELAM РёР· Р‘Р”.
///
/// # Returns
/// - `(expected, received)`:
///   - РґР»СЏ summary: `expected <- addr_human`, `received <- count_words`;
///   - РґР»СЏ РѕР±С‹С‡РЅРѕР№ СЃС‚СЂРѕРєРё: `expected <- count_words`, `received <- decode(resp)`.
///
/// # РџСЂРёРјРµСЂ
/// - `let (exp, rcv) = elam_expected_received(row);`
fn elam_expected_received(row: &ElamRow) -> (Option<usize>, Option<usize>) {
    if elam_is_summary(row) {
        let expected = row.addr_human.and_then(|v| if v >= 0 { Some(v as usize) } else { None });
        let received = row.count_words.and_then(|v| if v >= 0 { Some(v as usize) } else { None });
        return (expected, received);
    }

    let expected = row.count_words.and_then(|v| if v >= 0 { Some(v as usize) } else { None });
    let received = row.resp.as_deref().and_then(rx_words_from_resp_packet);
    (expected, received)
}

/// Function: $name.
fn hex_join(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Function: $name.
fn fmt_num_compact(v: f64) -> String {
    let mut s = format!("{:.6}", v);
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') || s.ends_with(',') {
        s.pop();
    }
    if s.is_empty() { "0".to_string() } else { s }
}

/// Р¤РѕСЂРјР°С‚РёСЂСѓРµС‚ РїР°РєРµС‚ ELAM РІ РєРѕРјРїР°РєС‚РЅСѓСЋ РёР»Рё РїРѕР»РЅСѓСЋ hex-СЃС‚СЂРѕРєСѓ.
///
/// # Parameters
/// - `bytes`: РёСЃС…РѕРґРЅС‹Р№ РїР°РєРµС‚.
/// - `full`: `true` вЂ” РїРѕРєР°Р·Р°С‚СЊ РІРµСЃСЊ РїР°РєРµС‚, `false` вЂ” РїСЂРµРІСЊСЋ.
/// - `header_len`: РґР»РёРЅР° РїСЂРµС„РёРєСЃР° Р·Р°РіРѕР»РѕРІРєР°, РѕС‚РґРµР»СЏРµРјРѕРіРѕ РІ РѕС‚РґРµР»СЊРЅС‹Р№ Р±Р»РѕРє.
///
/// # Returns
/// - РЎС‚СЂРѕРєР° РІРёРґР° `{header} {payload}` СЃ РІРѕР·РјРѕР¶РЅС‹Рј СЃСѓС„С„РёРєСЃРѕРј `...` РїСЂРё СѓСЃРµС‡РµРЅРёРё.
///
/// # РџСЂРёРјРµСЂ
/// - `let s = format_elam_packet(&row.req, false, 22);`
fn format_elam_packet(bytes: &[u8], full: bool, header_len: usize) -> String {
    if bytes.is_empty() {
        return "<empty>".to_string();
    }

    let shown_len = if full { bytes.len() } else { bytes.len().min(32) };
    let shown = &bytes[..shown_len];
    let suffix = if !full && shown_len < bytes.len() { " ..." } else { "" };

    if shown.len() > header_len {
        let head = hex_join(&shown[..header_len]);
        let body = hex_join(&shown[header_len..]);
        return format!("{{{}}} {{{}}}{}", head, body, suffix);
    }

    format!("{{{}}}{}", hex_join(shown), suffix)
}

#[derive(Clone, Debug)]
struct IoConn {
    ip: String,
    port: u16,
    rtu: u16,
    modem: u16,
    kan: u8,
    speed: u8,
    stop: u8,
    par: u8,
    data: u8,
    max_pkt_len: usize,
}

#[derive(Clone, Copy, Debug)]
struct PreCmd {
    addr_human: i32,
    cnt_words: i32,
}

/// Function: $name.
fn next_packet_id() -> u8 {
    static PID: AtomicU8 = AtomicU8::new(0);
    PID.fetch_add(1, Ordering::Relaxed)
}

/// Function: $name.
fn send_mb_over_udp(conn: &IoConn, mb: &[u8], timeout: Duration) -> (Vec<u8>, Result<Vec<u8>, String>) {
    let pid = next_packet_id();
    let par = modbus::UdpParams {
        kan: conn.kan,
        speed: conn.speed,
        stop: conn.stop,
        par: conn.par,
        data: conn.data,
        rtu: conn.rtu,
        modem: conn.modem,
        port: conn.port,
        ip: conn.ip.clone(),
        packet_id: pid,
        pkt_type: 0,
        ..Default::default()
    };
    let header = modbus::shab(&par, 22 + mb.len());
    let mut tx: Vec<u8> = Vec::with_capacity(22 + mb.len());
    tx.extend_from_slice(&header);
    tx.extend_from_slice(mb);

    let sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => return (tx, Err(format!("udp bind failed: {e}"))),
    };
    if let Err(e) = sock.set_read_timeout(Some(timeout)) {
        return (tx, Err(format!("udp timeout set failed: {e}")));
    }
    if let Err(e) = sock.send_to(&tx, format!("{}:{}", conn.ip, conn.port)) {
        return (tx, Err(format!("udp send failed: {e}")));
    }

    let mut buf = vec![0u8; conn.max_pkt_len.max(65535)];
    loop {
        let (n, _) = match sock.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e) => return (tx, Err(format!("udp recv failed: {e}"))),
        };
        if n < 12 {
            continue;
        }
        let pkt = &buf[..n];
        if pkt[3] != pid || pkt[4] != 1 {
            continue;
        }
        return (tx, Ok(pkt.to_vec()));
    }
}

/// Function: $name.
fn parse_read_words_from_resp(resp: &[u8], expected_func: u8) -> Result<Vec<u16>, String> {
    let mb = modbus::extract_modbus_frame(resp).ok_or_else(|| "short response".to_string())?;
    if mb.len() <= 4 {
        return Err("short response".to_string());
    }
    let ulen = if mb[0] >= 0xF8 { 2 } else { 1 };
    let fi = ulen;
    if mb.len() <= fi + 1 {
        return Err("bad response frame".to_string());
    }
    let func = mb[fi];
    if (func & 0x80) != 0 {
        return Err(format!("modbus exception func={func}"));
    }
    if func != expected_func {
        return Err(format!("unexpected func={}, expected={}", func, expected_func));
    }
    let (byte_count, data_start) = if ulen == 2 {
        if mb.len() < fi + 3 {
            return Err("short 2-byte-len frame".to_string());
        }
        ((((mb[fi + 1] as usize) << 8) | (mb[fi + 2] as usize)), fi + 3)
    } else {
        (mb[fi + 1] as usize, fi + 2)
    };
    if byte_count == 0 || (byte_count % 2) != 0 {
        return Err(format!("bad byte_count={}", byte_count));
    }
    if mb.len() < data_start + byte_count {
        return Err("short data".to_string());
    }
    let mut out = Vec::with_capacity(byte_count / 2);
    let data = &mb[data_start..data_start + byte_count];
    for i in 0..(byte_count / 2) {
        let hi = data[i * 2] as u16;
        let lo = data[i * 2 + 1] as u16;
        out.push((hi << 8) | lo);
    }
    Ok(out)
}

/// Function: $name.
fn validate_modbus_response(resp: &[u8], expected_func: u8) -> Result<(), String> {
    let mb = modbus::extract_modbus_frame(resp).ok_or_else(|| "short response".to_string())?;
    if mb.len() <= 4 {
        return Err("short modbus response".to_string());
    }
    let ulen = if mb[0] >= 0xF8 { 2 } else { 1 };
    if mb.len() <= ulen {
        return Err("bad response frame".to_string());
    }
    let func = mb[ulen];
    if (func & 0x80) != 0 {
        return Err(format!("modbus exception func={func}"));
    }
    if func != expected_func {
        return Err(format!("unexpected func={}, expected={}", func, expected_func));
    }
    Ok(())
}

/// Function: $name.
fn decode_pre_cmds(regs_out: &HashMap<i32, f64>, max_k: i32, max_words: i32) -> Vec<PreCmd> {
    let mut out = Vec::new();
    for k in 0..max_k {
        let base = 1000 + k * 3;
        let en = regs_out.get(&base).copied().unwrap_or(0.0) as i32;
        if en == 0 {
            continue;
        }
        let adr = regs_out.get(&(base + 1)).copied().unwrap_or(0.0) as i32;
        let cnt = regs_out.get(&(base + 2)).copied().unwrap_or(0.0) as i32;
        if adr <= 0 || cnt <= 0 || cnt > max_words {
            continue;
        }
        out.push(PreCmd {
            addr_human: adr,
            cnt_words: cnt,
        });
    }
    out
}

const IO_REQ_TIMEOUT_MS: u64 = 1200;
const IO_MAX_ROWS_PER_CLICK: usize = 120;
impl Ss5App {
    /// Function: $name.
    pub fn try_new() -> Result<Self> {
        let db = Db::connect_from_env()?;
        let mut app = Self {
            db,
            kpz: Vec::new(),
            groups: Vec::new(),
            obj_rows: Vec::new(),
            ref_ip: HashMap::new(),
            ref_port: HashMap::new(),
            ref_speed: HashMap::new(),
            ref_parit: HashMap::new(),
            ref_bit: HashMap::new(),
            ref_stop: HashMap::new(),
            ref_kanal: HashMap::new(),
            ref_n_mb: HashMap::new(),
            ref_tip: HashMap::new(),
            ref_c: HashMap::new(),
            selected_kpz: None,
            kpz_filter: String::new(),
            show_all_kpz: false,
            start_flag: false,
            poll_log: Vec::new(),
            elam: Vec::new(),
            poll_log_limit: 200,
            elam_limit: 50,
            elam_ts_from: String::new(),
            elam_ts_to: String::new(),
            elam_time_picker_open: false,
            elam_time_picker_target: ElamTimeTarget::From,
            elam_time_picker_year: 2026,
            elam_time_picker_month: 1,
            elam_time_picker_day: 1,
            elam_time_picker_hour: 0,
            elam_time_picker_min: 0,
            elam_time_picker_sec: 0,
            load_err: None,
            kpz_save_status: None,
            auto_refresh_every: Duration::from_secs(2),
            last_auto_refresh: Instant::now() - Duration::from_secs(10),
            kpz_t_a_edit: String::new(),
            kpz_t_script_edit: String::new(),
            selected_elam_index: None,
            show_elam_details: false,
            elam_full_hex: false,
            group_editor_open: false,
            group_edit_selected: BTreeSet::new(),
            group_edit_filter: String::new(),
            group_edit_err: None,
            group_edit_dirty: false,
            gscript_open: false,
            gscript_output_open: false,
            gscript_group_id: None,
            gscript_group_ids_db: Vec::new(),
            gscript_templates: Vec::new(),
            gscript_template_id: None,
            gscript_template_name: String::new(),
            gscript_pre: String::new(),
            gscript_post: String::new(),
            gscript_elam: false,
            gscript_max_words: 800,
            gscript_max_k: 2,
            gscript_en: true,
            gscript_ver: 1,
            gscript_status: None,
            gscript_err: None,
            gscript_dirty: false,
            gscript_words_json: "[]".to_string(),
            gscript_regs_json: "{\"70010\":1000800000}".to_string(),
            gscript_hi_lo: true,
            gscript_print_log: String::new(),
            gscript_regs_out: Vec::new(),
            gscript_emits_out: Vec::new(),
            gscript_use_rv_fallback: true,
            gscript_tab: GScriptTab::Pre,
            gscript_output_tab: GScriptOutputTab::Print,
            gscript_help_open: false,
            graph_open: false,
            graph_group_id: None,
            graph_regs: Vec::new(),
            graph_selected_regs: BTreeSet::new(),
            graph_series: Vec::new(),
            graph_window_sec: 86_400,
            graph_limit: 1500,
            graph_err: None,
            kpz_io_open: false,
            kpz_io_group_id: None,
            kpz_io_input_rows: Vec::new(),
            kpz_io_holding_rows: Vec::new(),
            kpz_io_selected_holding: None,
            kpz_io_write_value: String::new(),
            kpz_io_cmd5_regs: Vec::new(),
            kpz_io_cmd5_addr: None,
            kpz_io_status: None,
            kpz_io_err: None,
            kpz_io_obj_info: String::new(),
            kpz_io_last_tx_hex: String::new(),
            kpz_io_last_rx_hex: String::new(),
            kpz_io_script_log: String::new(),
            kpz_io_cached_vals: HashMap::new(),
            kpz_editor_open: false,
            kpz_editor_id: String::new(),
            kpz_editor_name: String::new(),
            kpz_editor_rtu: String::new(),
            kpz_editor_obj: None,
            kpz_editor_modem: String::new(),
            kpz_editor_max_pkt_len: "800".to_string(),
            kpz_editor_start: false,
            kpz_editor_en_post: false,
            kpz_editor_t_a: String::new(),
            kpz_editor_t_script: String::new(),
            kpz_editor_status: None,
            kpz_editor_err: None,
            obj_editor_open: false,
            obj_editor_selected_id: None,
            obj_editor_name: String::new(),
            obj_editor_ip: None,
            obj_editor_port: None,
            obj_editor_kanal: None,
            obj_editor_speed: None,
            obj_editor_stop: None,
            obj_editor_parit: None,
            obj_editor_bit: None,
            obj_editor_status: None,
            obj_editor_err: None,
            reg_editor_open: false,
            reg_rows: Vec::new(),
            reg_editor_selected_id: None,
            reg_editor_id: String::new(),
            reg_editor_group_filter: None,
            reg_editor_filter: String::new(),
            reg_editor_name: String::new(),
            reg_editor_mb: String::new(),
            reg_editor_n_mb: String::new(),
            reg_editor_tip: String::new(),
            reg_editor_bits: String::new(),
            reg_editor_grup: String::new(),
            reg_editor_a_en: false,
            reg_editor_a_no_write: String::new(),
            reg_editor_status: None,
            reg_editor_err: None,
            dict_editor_open: false,
            dict_table: "ip".to_string(),
            dict_items: Vec::new(),
            dict_editor_selected_id: None,
            dict_editor_id: String::new(),
            dict_editor_name: String::new(),
            dict_editor_status: None,
            dict_editor_err: None,
            alarm_open: false,
            alarm_rules: Vec::new(),
            alarm_state_rows: Vec::new(),
            alarm_events: Vec::new(),
            alarm_selected_rule_id: None,
            alarm_rule_kpz_id: String::new(),
            alarm_group_id: None,
            alarm_group_regs: Vec::new(),
            alarm_rule_reg_id: String::new(),
            alarm_rule_enabled: true,
            alarm_rule_cmp: "gt".to_string(),
            alarm_rule_set_lo: String::new(),
            alarm_rule_set_hi: String::new(),
            alarm_rule_set_lo_1: String::new(),
            alarm_rule_set_hi_1: String::new(),
            alarm_rule_hysteresis: "0".to_string(),
            alarm_rule_on_delay: "0".to_string(),
            alarm_rule_off_delay: "0".to_string(),
            alarm_rule_severity: "1".to_string(),
            alarm_rule_code: String::new(),
            alarm_rule_message: String::new(),
            alarm_rule_chat_id: String::new(),
            alarm_tg_on_on: false,
            alarm_tg_on_off: false,
            alarm_tg_thr_main: false,
            alarm_tg_thr_lvl1: false,
            alarm_events_limit: 200,
            alarm_state_limit: 200,
            alarm_status: None,
            alarm_err: None,
            range_kpz_open: false,
            range_kpz_id_start: "1200".to_string(),
            range_kpz_id_end: "1299".to_string(),
            range_kpz_obj: Some(5),
            range_kpz_modem_start: "5200".to_string(),
            range_kpz_t_a: String::new(),
            range_kpz_t_script: String::new(),
            range_kpz_groups_selected: BTreeSet::new(),
            range_kpz_start_enabled: false,
            range_kpz_help_open: false,
            range_kpz_status: None,
            range_kpz_err: None,
            runtime_cfg_open: false,
            runtime_no_resp_failures: "3".to_string(),
            runtime_no_resp_backoff_sec: "600".to_string(),
            runtime_metrics_p95_warn_ms: "1000".to_string(),
            runtime_metrics_p95_crit_ms: "3000".to_string(),
            runtime_modbus_a_timeout_ms: "1800".to_string(),
            runtime_modbus_script_timeout_ms: "2600".to_string(),
            runtime_cfg_status: None,
            runtime_cfg_err: None,
            arx_state_open: false,
            arx_state_rows: Vec::new(),
            arx_state_limit: 200,
            arx_state_kpz_filter: String::new(),
            arx_state_kpz_id: String::new(),
            arx_state_arx_id: String::new(),
            arx_state_last_ind: String::new(),
            arx_state_status: None,
            arx_state_err: None,
            arx_val_open: false,
            arx_val_rows: Vec::new(),
            arx_val_limit: 20,
            arx_val_kpz_filter: String::new(),
            arx_val_status: None,
            arx_val_err: None,
            ui_link_editor: UiLinkEditorState::default(),
            ui_log: Vec::new(),
        };
        app.push_log("ss5 started");
        app.reload_all();
        Ok(app)
    }

    /// Function: $name.
    fn push_log(&mut self, msg: impl Into<String>) {
        let ts = Local::now().format("%H:%M:%S").to_string();
        self.ui_log.push(format!("[{}] {}", ts, msg.into()));
        const MAX_LINES: usize = 400;
        if self.ui_log.len() > MAX_LINES {
            let drop_n = self.ui_log.len() - MAX_LINES;
            self.ui_log.drain(0..drop_n);
        }
    }

    /// Function: $name.
    fn sync_kpz_editor_from_selected(&mut self) {
        if let Some(row) = self
            .selected_kpz
            .and_then(|id| self.kpz.iter().find(|k| k.id == id))
        {
            self.start_flag = row.start == 1;
            self.kpz_t_a_edit = row.t_a.clone();
            self.kpz_t_script_edit = row.t_script.clone();
        } else {
            self.start_flag = false;
            self.kpz_t_a_edit.clear();
            self.kpz_t_script_edit.clear();
        }
        self.kpz_save_status = None;
    }

    /// Function: $name.
    fn sync_kpz_full_editor_from_selected(&mut self) {
        if let Some(row) = self
            .selected_kpz
            .and_then(|id| self.kpz.iter().find(|k| k.id == id))
        {
            self.kpz_editor_id = row.id.to_string();
            self.kpz_editor_name = row.name.clone();
            self.kpz_editor_rtu = row.rtu.to_string();
            self.kpz_editor_obj = Some(row.obj);
            self.kpz_editor_modem = row.modem.map(|v| v.to_string()).unwrap_or_default();
            self.kpz_editor_max_pkt_len = row.max_pkt_len.unwrap_or(800).to_string();
            self.kpz_editor_start = row.start == 1;
            self.kpz_editor_en_post = row.en_post;
            self.kpz_editor_t_a = row.t_a.clone();
            self.kpz_editor_t_script = row.t_script.clone();
        } else {
            self.kpz_editor_id.clear();
            self.kpz_editor_name.clear();
            self.kpz_editor_rtu.clear();
            self.kpz_editor_obj = None;
            self.kpz_editor_modem.clear();
            self.kpz_editor_max_pkt_len = "800".to_string();
            self.kpz_editor_start = false;
            self.kpz_editor_en_post = false;
            self.kpz_editor_t_a.clear();
            self.kpz_editor_t_script.clear();
        }
        self.kpz_editor_err = None;
        self.kpz_editor_status = None;
    }

    /// Function: $name.
    fn save_quick_kpz_meta_for_selected(&mut self) -> Result<bool, String> {
        let Some(id) = self.selected_kpz else {
            return Ok(false);
        };
        let Some(cur) = self.kpz.iter().find(|k| k.id == id).cloned() else {
            return Ok(false);
        };

        let start = if self.start_flag { 1 } else { 0 };
        let t_a = {
            let s = self.kpz_t_a_edit.trim();
            if s.is_empty() {
                None
            } else {
                Some(
                    s.parse::<i32>()
                        .map_err(|_| "t_a должен быть целым числом или пустым".to_string())?,
                )
            }
        };
        let t_script = {
            let s = self.kpz_t_script_edit.trim();
            if s.is_empty() {
                None
            } else {
                Some(
                    s.parse::<i32>()
                        .map_err(|_| "t_pre/t_script должен быть целым числом или пустым".to_string())?,
                )
            }
        };

        let cur_t_a = if cur.t_a.trim().is_empty() {
            None
        } else {
            cur.t_a.trim().parse::<i32>().ok()
        };
        let cur_t_script = if cur.t_script.trim().is_empty() {
            None
        } else {
            cur.t_script.trim().parse::<i32>().ok()
        };

        if cur.start == start && cur_t_a == t_a && cur_t_script == t_script {
            return Ok(false);
        }

        self.db
            .update_kpz_meta(id, start, t_a, t_script)
            .map_err(|e| format!("update_kpz_meta failed: {e}"))?;

        if let Some(row) = self.kpz.iter_mut().find(|k| k.id == id) {
            row.start = start;
            row.t_a = self.kpz_t_a_edit.trim().to_string();
            row.t_script = self.kpz_t_script_edit.trim().to_string();
        }
        self.kpz_save_status = Some("Сохранено".to_string());
        Ok(true)
    }

    /// Function: $name.
    fn effective_kpz_filter(&self) -> Option<i32> {
        // Preserve focused diagnostics: when selected KPZ is in start mode,
        // always show logs for that KPZ even if "all KPZ" view is enabled.
        if self.start_flag {
            self.selected_kpz
        } else if self.show_all_kpz {
            None
        } else {
            self.selected_kpz
        }
    }

    /// Function: $name.
    fn selected_kpz_name(&self) -> String {
        self.selected_kpz
            .and_then(|id| self.kpz.iter().find(|k| k.id == id))
            .map(|k| format!("{} - {}", k.id, k.name))
            .unwrap_or_else(|| "<none>".to_string())
    }

    /// Function: $name.
    fn selected_kpz_grups(&self) -> Vec<u8> {
        self.selected_kpz
            .and_then(|id| self.kpz.iter().find(|k| k.id == id))
            .map(|k| k.grups.clone())
            .unwrap_or_else(|| vec![0u8; 64])
    }

    /// Function: $name.
    fn selected_enabled_groups(&self) -> Vec<i32> {
        decode_groups(&self.selected_kpz_grups())
    }

    /// Function: $name.
    fn set_error(&mut self, e: impl ToString) {
        let msg = e.to_string();
        self.load_err = Some(msg.clone());
        self.push_log(format!("ERROR: {}", msg));
    }

    /// Function: $name.
    fn reload_all(&mut self) {
        match self.db.get_all_kpz() {
            Ok(kpz) => {
                self.kpz = kpz;
                if self.selected_kpz.is_none() {
                    self.selected_kpz = self.kpz.first().map(|v| v.id);
                }
                self.sync_kpz_editor_from_selected();
                self.sync_kpz_full_editor_from_selected();
                self.reload_groups();
                self.reload_kpz_refs();
                self.reload_logs();
                self.load_err = None;
            }
            Err(e) => self.set_error(format!("get_all_kpz failed: {e}")),
        }
    }

    /// Function: $name.
    fn load_ref_map(&self, table: &str) -> Result<HashMap<i32, String>> {
        let items = self.db.get_items(table)?;
        Ok(items.into_iter().map(|v| (v.id, v.name)).collect())
    }

    /// Function: $name.
    fn reload_kpz_refs(&mut self) {
        match self.db.get_all_obj() {
            Ok(v) => self.obj_rows = v,
            Err(e) => self.set_error(format!("get_all_obj failed: {e}")),
        }
        match self.load_ref_map("ip") {
            Ok(v) => self.ref_ip = v,
            Err(e) => self.set_error(format!("get_items(ip) failed: {e}")),
        }
        match self.load_ref_map("port") {
            Ok(v) => self.ref_port = v,
            Err(e) => self.set_error(format!("get_items(port) failed: {e}")),
        }
        match self.load_ref_map("speed") {
            Ok(v) => self.ref_speed = v,
            Err(e) => self.set_error(format!("get_items(speed) failed: {e}")),
        }
        match self.load_ref_map("parit") {
            Ok(v) => self.ref_parit = v,
            Err(e) => self.set_error(format!("get_items(parit) failed: {e}")),
        }
        match self.load_ref_map("bit") {
            Ok(v) => self.ref_bit = v,
            Err(e) => self.set_error(format!("get_items(bit) failed: {e}")),
        }
        match self.load_ref_map("stop") {
            Ok(v) => self.ref_stop = v,
            Err(e) => self.set_error(format!("get_items(stop) failed: {e}")),
        }
        match self.load_ref_map("kanal") {
            Ok(v) => self.ref_kanal = v,
            Err(e) => self.set_error(format!("get_items(kanal) failed: {e}")),
        }
        match self.load_ref_map("n_mb") {
            Ok(v) => self.ref_n_mb = v,
            Err(_) => self.ref_n_mb.clear(),
        }
        match self.load_ref_map("tip") {
            Ok(v) => self.ref_tip = v,
            Err(_) => self.ref_tip.clear(),
        }
        match self.load_ref_map("bits") {
            Ok(v) => self.ref_c = v,
            Err(_) => match self.load_ref_map("c") {
                Ok(v) => self.ref_c = v,
                Err(_) => self.ref_c.clear(),
            },
        }
    }

    /// Function: $name.
    fn reload_groups(&mut self) {
        match self.db.get_all_groups() {
            Ok(groups) => self.groups = groups,
            Err(e) => self.set_error(format!("get_all_groups failed: {e}")),
        }
    }

    /// Function: $name.
    fn reload_logs(&mut self) {
        let filter = self.effective_kpz_filter();
        let ts_from_unix = match parse_time_filter_to_unix_sec(&self.elam_ts_from) {
            Ok(v) => v,
            Err(e) => {
                self.set_error(format!("ELAM РёРЅС‚РµСЂРІР°Р» 'РЎ': {e}"));
                return;
            }
        };
        let ts_to_unix = match parse_time_filter_to_unix_sec(&self.elam_ts_to) {
            Ok(v) => v,
            Err(e) => {
                self.set_error(format!("ELAM РёРЅС‚РµСЂРІР°Р» 'РџРѕ': {e}"));
                return;
            }
        };
        if let (Some(ts_from), Some(ts_to)) = (ts_from_unix, ts_to_unix) {
            if ts_from > ts_to {
                self.set_error("ELAM РёРЅС‚РµСЂРІР°Р»: 'РЎ' Р±РѕР»СЊС€Рµ 'РџРѕ'");
                return;
            }
        }
        match self.db.get_poll_log(filter, self.poll_log_limit) {
            Ok(v) => self.poll_log = v,
            Err(e) => self.set_error(format!("get_poll_log failed: {e}")),
        }
        match self
            .db
            .get_last_elam(filter, self.elam_limit, ts_from_unix, ts_to_unix)
        {
            Ok(v) => self.elam = v,
            Err(e) => self.set_error(format!("get_last_elam failed: {e}")),
        }
    }

    /// Function: $name.
    fn open_elam_time_picker(&mut self, target: ElamTimeTarget) {
        self.elam_time_picker_target = target;
        self.elam_time_picker_open = true;
        let source = match target {
            ElamTimeTarget::From => self.elam_ts_from.trim(),
            ElamTimeTarget::To => self.elam_ts_to.trim(),
        };
        let ts = match parse_time_filter_to_unix_sec(source) {
            Ok(Some(v)) => v,
            _ => Local::now().timestamp(),
        };
        if let LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) = Local.timestamp_opt(ts, 0) {
            self.elam_time_picker_year = dt.year();
            self.elam_time_picker_month = dt.month();
            self.elam_time_picker_day = dt.day();
            self.elam_time_picker_hour = dt.hour();
            self.elam_time_picker_min = dt.minute();
            self.elam_time_picker_sec = dt.second();
        }
    }

    /// Function: $name.
    fn set_elam_time_picker_now(&mut self) {
        let now = Local::now();
        self.elam_time_picker_year = now.year();
        self.elam_time_picker_month = now.month();
        self.elam_time_picker_day = now.day();
        self.elam_time_picker_hour = now.hour();
        self.elam_time_picker_min = now.minute();
        self.elam_time_picker_sec = now.second();
    }

    /// Function: $name.
    fn set_elam_time_picker_day_start(&mut self) {
        let now = Local::now();
        self.elam_time_picker_year = now.year();
        self.elam_time_picker_month = now.month();
        self.elam_time_picker_day = now.day();
        self.elam_time_picker_hour = 0;
        self.elam_time_picker_min = 0;
        self.elam_time_picker_sec = 0;
    }

    /// Function: $name.
    fn apply_elam_time_picker_to_field(&mut self) -> Result<(), String> {
        let date = NaiveDate::from_ymd_opt(
            self.elam_time_picker_year,
            self.elam_time_picker_month.clamp(1, 12),
            self.elam_time_picker_day.clamp(1, 31),
        )
        .ok_or_else(|| "РќРµРєРѕСЂСЂРµРєС‚РЅР°СЏ РґР°С‚Р°".to_string())?;
        let naive = date
            .and_hms_opt(
                self.elam_time_picker_hour.clamp(0, 23),
                self.elam_time_picker_min.clamp(0, 59),
                self.elam_time_picker_sec.clamp(0, 59),
            )
            .ok_or_else(|| "РќРµРєРѕСЂСЂРµРєС‚РЅРѕРµ РІСЂРµРјСЏ".to_string())?;
        let dt = match Local.from_local_datetime(&naive) {
            LocalResult::Single(v) => v,
            LocalResult::Ambiguous(v, _) => v,
            LocalResult::None => return Err("Р’С‹Р±СЂР°РЅРЅРѕРµ РІСЂРµРјСЏ РЅРµРґРѕСЃС‚СѓРїРЅРѕ РІ Р»РѕРєР°Р»СЊРЅРѕР№ Р·РѕРЅРµ".to_string()),
        };
        let text = dt.format("%Y-%m-%d %H:%M:%S").to_string();
        match self.elam_time_picker_target {
            ElamTimeTarget::From => self.elam_ts_from = text,
            ElamTimeTarget::To => self.elam_ts_to = text,
        }
        Ok(())
    }
    /// Function: $name.
    fn ref_caption(map: &HashMap<i32, String>, id: Option<i32>) -> String {
        match id {
            Some(v) => {
                let name = map
                    .get(&v)
                    .cloned()
                    .unwrap_or_else(|| "<unknown>".to_string());
                format!("{} - {}", v, name)
            }
            None => "-".to_string(),
        }
    }

    #[allow(dead_code)]
    /// Function: $name.
    fn sorted_ref_pairs(map: &HashMap<i32, String>) -> Vec<(i32, String)> {
        let mut out: Vec<(i32, String)> = map.iter().map(|(k, v)| (*k, v.clone())).collect();
        out.sort_by_key(|(id, _)| *id);
        out
    }

    /// Function: $name.
    fn obj_caption(&self, id: Option<i32>) -> String {
        match id {
            Some(v) => self
                .obj_rows
                .iter()
                .find(|o| o.id == v)
                .map(|o| {
                    if o.name.is_empty() {
                        o.id.to_string()
                    } else {
                        format!("{} - {}", o.id, o.name)
                    }
                })
                .unwrap_or_else(|| format!("{} - <unknown>", v)),
            None => "<none>".to_string(),
        }
    }

    #[allow(dead_code)]
    /// Function: $name.
    fn group_caption(&self, id: Option<i32>) -> String {
        match id {
            Some(v) => self
                .groups
                .iter()
                .find(|g| g.id == v)
                .map(|g| {
                    if g.name.is_empty() {
                        g.id.to_string()
                    } else {
                        format!("{} - {}", g.id, g.name)
                    }
                })
                .unwrap_or_else(|| format!("{} - <unknown>", v)),
            None => "All groups".to_string(),
        }
    }

    /// Function: $name.
    fn open_ui_link_editor(&mut self) {
        self.ui_link_editor.open = true;
        self.ui_link_editor.err = None;
        self.ui_link_editor.status = None;
        self.ui_link_reload_windows();
    }

    /// Function: $name.
    fn ui_link_reload_windows(&mut self) {
        let Some(kpz_id) = self.selected_kpz else {
            self.ui_link_editor.err = Some("Select KPZ first".to_string());
            return;
        };
        match self.db.get_ui_kpz_windows(kpz_id) {
            Ok(rows) => {
                self.ui_link_editor.windows = rows;
                self.ui_link_editor.err = None;
                self.ui_link_editor.status = Some("windows loaded".to_string());
            }
            Err(e) => {
                self.ui_link_editor.err = Some(format!("load windows failed: {e}"));
            }
        }
    }

    /// Function: $name.
    fn ui_link_upsert_window(&mut self) {
        let Some(kpz_id) = self.selected_kpz else {
            self.ui_link_editor.err = Some("Select KPZ first".to_string());
            return;
        };
        let code = self.ui_link_editor.window_code.trim().to_string();
        let title = self.ui_link_editor.window_title.trim().to_string();
        if code.is_empty() || title.is_empty() {
            self.ui_link_editor.err = Some("window code/title are required".to_string());
            return;
        }
        let desc = self.ui_link_editor.window_description.trim().to_string();
        let desc_opt = if desc.is_empty() { None } else { Some(desc.as_str()) };
        match self
            .db
            .upsert_ui_kpz_window(kpz_id, &code, &title, desc_opt, true)
        {
            Ok(window_id) => {
                self.ui_link_editor.selected_window_id = Some(window_id);
                self.ui_link_editor.err = None;
                self.ui_link_editor.status = Some(format!("window saved: {} [{}]", title, code));
                self.ui_link_reload_windows();
                self.ui_link_load_window(Some(window_id));
            }
            Err(e) => {
                self.ui_link_editor.err = Some(format!("save window failed: {e}"));
            }
        }
    }

    /// Function: $name.
    fn ui_link_load_window(&mut self, window_id: Option<i64>) {
        self.ui_link_editor.selected_window_id = window_id;
        self.ui_link_editor.groups_selected.clear();
        self.ui_link_editor.bindings.clear();
        self.ui_link_editor.regs_available.clear();
        self.ui_link_editor.regs_selected.clear();
        self.ui_link_editor.live_values.clear();
        self.ui_link_editor.cmd_inputs.clear();
        self.ui_link_editor.last_cmd_result.clear();
        self.ui_link_editor.preview_edit_reg_id = None;
        self.ui_link_editor.dirty = false;
        self.ui_link_editor.err = None;

        let Some(id) = window_id else { return; };
        if let Some(w) = self.ui_link_editor.windows.iter().find(|w| w.id == id) {
            self.ui_link_editor.window_code = w.code.clone();
            self.ui_link_editor.window_title = w.title.clone();
            self.ui_link_editor.window_description = w.description.clone().unwrap_or_default();
        }

        match self.db.get_ui_window_groups(id) {
            Ok(gs) => {
                for g in gs {
                    self.ui_link_editor.groups_selected.insert(g.group_id);
                }
            }
            Err(e) => {
                self.ui_link_editor.err = Some(format!("load groups failed: {e}"));
                return;
            }
        }
        match self.db.get_ui_window_bindings(id) {
            Ok(bs) => {
                self.ui_link_editor.bindings = bs;
            }
            Err(e) => {
                self.ui_link_editor.err = Some(format!("load bindings failed: {e}"));
                return;
            }
        }
        self.ui_link_reload_regs();
    }

    /// Function: $name.
    fn ui_link_reload_regs(&mut self) {
        let ids: Vec<i32> = self.ui_link_editor.groups_selected.iter().copied().collect();
        match self.db.get_regs_by_groups(&ids) {
            Ok(rows) => {
                self.ui_link_editor.regs_available = rows;
                self.ui_link_editor.err = None;
                self.ui_link_editor.status = Some(format!(
                    "loaded regs for {} groups",
                    self.ui_link_editor.groups_selected.len()
                ));
            }
            Err(e) => {
                self.ui_link_editor.err = Some(format!("load regs failed: {e}"));
            }
        }
    }

    /// Function: $name.
    fn ui_link_save_all(&mut self) {
        let Some(window_id) = self.ui_link_editor.selected_window_id else {
            self.ui_link_editor.err = Some("Select/save window first".to_string());
            return;
        };

        let mut groups: Vec<UiWindowGroupRow> = self
            .ui_link_editor
            .groups_selected
            .iter()
            .enumerate()
            .map(|(i, gid)| UiWindowGroupRow {
                group_id: *gid,
                pos: ((i as i32) + 1) * 10,
            })
            .collect();
        groups.sort_by_key(|g| g.pos);

        let mut bindings = self.ui_link_editor.bindings.clone();
        bindings.sort_by_key(|b| b.pos);
        for (i, b) in bindings.iter_mut().enumerate() {
            b.pos = ((i as i32) + 1) * 10;
        }

        if let Err(e) = self.db.save_ui_window_groups(window_id, &groups) {
            self.ui_link_editor.err = Some(format!("save groups failed: {e}"));
            return;
        }
        if let Err(e) = self.db.save_ui_window_bindings(window_id, &bindings) {
            self.ui_link_editor.err = Some(format!("save bindings failed: {e}"));
            return;
        }

        self.ui_link_editor.bindings = bindings;
        self.ui_link_editor.err = None;
        self.ui_link_editor.status = Some(format!(
            "saved: groups={}, bindings={}",
            groups.len(),
            self.ui_link_editor.bindings.len()
        ));
        self.ui_link_editor.dirty = false;
    }

    /// Function: $name.
    fn ui_link_read_func_by_n_mb(&self, n_mb_id: i32) -> u8 {
        let name = self
            .ref_n_mb
            .get(&n_mb_id)
            .map(|s| s.trim().to_uppercase())
            .unwrap_or_default();
        if name.contains("TIT") { 4 } else { 3 }
    }

    /// Function: $name.
    fn ui_link_binding_to_io_row(binding: &crate::models::UiWindowBindingRow) -> KpzIoRow {
        KpzIoRow {
            id: binding.reg_id,
            name: binding.reg_name.clone(),
            mb: binding.reg_mb,
            tip: binding.reg_tip,
            bits: binding.reg_bits,
            reg_val: None,
            last_val: None,
        }
    }

    fn ui_link_parse_binding_value_from_words(
        binding: &crate::models::UiWindowBindingRow,
        words: &[u16],
    ) -> Result<f64, String> {
        if words.is_empty() {
            return Err("empty words".to_string());
        }
        if matches!(binding.reg_tip, 2 | 4 | 5) {
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
            let v = match binding.reg_tip {
                5 => f32::from_be_bytes(bytes) as f64,
                4 => u32::from_be_bytes(bytes) as f64,
                2 => i32::from_be_bytes(bytes) as f64,
                _ => u32::from_be_bytes(bytes) as f64,
            };
            return Ok(v);
        }
        if binding.reg_tip == 0
            && let Some(bit) = binding.reg_bits
            && (0..=15).contains(&bit)
        {
            return Ok(((words[0] >> bit) & 1) as f64);
        }
        let w = words[0];
        let v = match binding.reg_tip {
            1 => (w as i16) as f64,
            _ => w as f64,
        };
        Ok(v)
    }

    fn ui_link_poll_now(&mut self) {
        self.ui_link_editor.err = None;
        self.ui_link_editor.status = None;
        let conn = match self.build_io_conn() {
            Ok(c) => c,
            Err(e) => {
                self.ui_link_editor.err = Some(e);
                return;
            }
        };
        self.ui_link_editor.live_values.clear();
        let bindings = self.ui_link_editor.bindings.clone();
        let poll_bindings: Vec<_> = bindings
            .into_iter()
            .filter(|b| b.visible && !(b.reg_n_mb == 1 || b.reg_tip == 1))
            .collect();
        let mut by_func: std::collections::BTreeMap<u8, Vec<crate::models::UiWindowBindingRow>> =
            std::collections::BTreeMap::new();
        for b in poll_bindings {
            by_func
                .entry(self.ui_link_read_func_by_n_mb(b.reg_n_mb))
                .or_default()
                .push(b);
        }

        let mut blocks: Vec<(u8, i32, i32, Vec<crate::models::UiWindowBindingRow>)> = Vec::new();
        for (func, mut regs) in by_func {
            regs.sort_by_key(|b| b.reg_mb);
            let mut cur: Vec<crate::models::UiWindowBindingRow> = Vec::new();
            let mut start = 0i32;
            let mut end = 0i32;
            for b in regs {
                let words = if matches!(b.reg_tip, 2 | 4 | 5) { 2 } else { 1 };
                let rs = b.reg_mb;
                let re = b.reg_mb + words - 1;
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

        if blocks.is_empty() {
            self.ui_link_editor.status = Some("no poll requests".to_string());
            return;
        }

        let reqs: Vec<modbus_service::ReadReq> = blocks
            .iter()
            .map(|(func, addr, cnt, _)| modbus_service::ReadReq {
                func: *func,
                addr_human: *addr,
                cnt_words: *cnt,
            })
            .collect();
        let service_conn = Self::as_service_conn(&conn);
        let idle_ms = ((reqs.len() as u64) * 25).clamp(60, 500);

        let mut ok = 0usize;
        let mut fail = 0usize;
        let mut first_err: Option<String> = None;
        match modbus_service::request_reqs_glued(
            &service_conn,
            &reqs,
            Duration::from_millis(IO_REQ_TIMEOUT_MS),
            Duration::from_millis(idle_ms),
        ) {
            Ok(multi) => {
                for (i, (func, addr, _cnt, regs)) in blocks.iter().enumerate() {
                    let Some(res) = multi.results.get(i) else {
                        for b in regs {
                            self.ui_link_editor.live_values.insert(b.reg_id, None);
                            fail += 1;
                            if first_err.is_none() {
                                first_err = Some(format!("reg {} mb {}: no response", b.reg_id, b.reg_mb));
                            }
                        }
                        continue;
                    };
                    let Some(vpkt) = &res.response else {
                        for b in regs {
                            self.ui_link_editor.live_values.insert(b.reg_id, None);
                            fail += 1;
                            if first_err.is_none() {
                                first_err = Some(format!("reg {} mb {}: empty response", b.reg_id, b.reg_mb));
                            }
                        }
                        continue;
                    };
                    match parse_read_words_from_resp(vpkt, *func) {
                        Ok(words) => {
                            for b in regs {
                                let off = (b.reg_mb - *addr) as usize;
                                let need = if matches!(b.reg_tip, 2 | 4 | 5) { 2 } else { 1 };
                                if off + need > words.len() {
                                    self.ui_link_editor.live_values.insert(b.reg_id, None);
                                    fail += 1;
                                    if first_err.is_none() {
                                        first_err = Some(format!(
                                            "reg {} mb {}: out of block bounds",
                                            b.reg_id, b.reg_mb
                                        ));
                                    }
                                    continue;
                                }
                                match Self::ui_link_parse_binding_value_from_words(b, &words[off..off + need]) {
                                    Ok(v) => {
                                        self.ui_link_editor.live_values.insert(b.reg_id, Some(v));
                                        ok += 1;
                                    }
                                    Err(e) => {
                                        self.ui_link_editor.live_values.insert(b.reg_id, None);
                                        fail += 1;
                                        if first_err.is_none() {
                                            first_err = Some(format!("reg {} mb {}: {}", b.reg_id, b.reg_mb, e));
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            for b in regs {
                                self.ui_link_editor.live_values.insert(b.reg_id, None);
                                fail += 1;
                                if first_err.is_none() {
                                    first_err = Some(format!("reg {} mb {}: {}", b.reg_id, b.reg_mb, e));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                for (_func, _addr, _cnt, regs) in &blocks {
                    for b in regs {
                        self.ui_link_editor.live_values.insert(b.reg_id, None);
                        fail += 1;
                        if first_err.is_none() {
                            first_err = Some(format!("reg {} mb {}: {}", b.reg_id, b.reg_mb, e));
                        }
                    }
                }
            }
        }
        self.ui_link_editor.status = Some(format!("poll IO done: ok={}, fail={}", ok, fail));
        self.ui_link_editor.err = first_err;
    }

    /// Function: $name.
    fn ui_link_send_tu(&mut self, reg_id: i32, on: bool) {
        self.ui_link_editor.err = None;
        self.ui_link_editor.status = None;
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
        let dat = if on { [0xFFu8, 0x00u8] } else { [0x00u8, 0x00u8] };
        let mb = match modbus::sout_mb_only(conn.rtu, 5, binding.reg_mb, 1, Some(&dat)) {
            Ok(v) => v,
            Err(e) => {
                self.ui_link_editor.err = Some(format!("fc5 build failed: {e}"));
                return;
            }
        };
        let (_, resp_res) = send_mb_over_udp(&conn, &mb, Duration::from_millis(IO_REQ_TIMEOUT_MS));
        match resp_res {
            Ok(resp) => match validate_modbus_response(&resp, 5) {
                Ok(()) => {
                    self.ui_link_editor
                        .last_cmd_result
                        .insert(reg_id, if on { "OK ON".to_string() } else { "OK OFF".to_string() });
                    self.ui_link_editor.status = Some(format!(
                        "FC5 {} OK reg={} mb={}",
                        if on { "ON" } else { "OFF" },
                        reg_id,
                        binding.reg_mb
                    ));
                }
                Err(e) => {
                    self.ui_link_editor.last_cmd_result.insert(reg_id, "ERR".to_string());
                    self.ui_link_editor.err = Some(format!("fc5 bad response for reg {}: {e}", reg_id));
                }
            },
            Err(e) => {
                self.ui_link_editor.last_cmd_result.insert(reg_id, "ERR".to_string());
                self.ui_link_editor.err = Some(format!("fc5 send failed for reg {}: {e}", reg_id));
            }
        }
    }

    /// Function: $name.
    fn ui_link_write_value(&mut self, reg_id: i32, val: f64) {
        self.ui_link_editor.err = None;
        self.ui_link_editor.status = None;
        let Some(binding) = self.ui_link_editor.bindings.iter().find(|b| b.reg_id == reg_id).cloned() else {
            self.ui_link_editor.err = Some(format!("binding for reg {} not found", reg_id));
            return;
        };
        if binding.reg_n_mb == 1 || binding.reg_tip == 1 {
            self.ui_link_editor.err = Some(format!(
                "reg {} is TU command (n_mb={}, tip={}), use FC5",
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
        let row = Self::ui_link_binding_to_io_row(&binding);
        match Self::write_reg_direct(&conn, &row, val) {
            Ok((_, resp)) => match validate_modbus_response(&resp, 16) {
                Ok(()) => {
                    self.ui_link_editor.last_cmd_result.insert(reg_id, "OK FC16".to_string());
                    self.ui_link_editor.live_values.insert(reg_id, Some(val));
                    self.ui_link_editor.status =
                        Some(format!("FC16 sent: reg {} mb={} value={:.3}", reg_id, binding.reg_mb, val));
                }
                Err(e) => {
                    self.ui_link_editor.last_cmd_result.insert(reg_id, "ERR".to_string());
                    self.ui_link_editor.err = Some(format!("fc16 bad response for reg {}: {e}", reg_id));
                }
            },
            Err(e) => {
                self.ui_link_editor.last_cmd_result.insert(reg_id, "ERR".to_string());
                self.ui_link_editor.err = Some(format!("write failed for reg {}: {e}", reg_id));
            }
        }
    }

    /// Function: $name.
    fn graph_groups_for_selected_kpz(&self) -> Vec<GroupRow> {
        let Some(kpz_id) = self.selected_kpz else {
            return Vec::new();
        };
        self.graph_groups_for_kpz(kpz_id)
    }

    /// Function: $name.
    fn graph_groups_for_kpz(&self, kpz_id: i32) -> Vec<GroupRow> {
        let enabled: BTreeSet<i32> = self
            .kpz
            .iter()
            .find(|k| k.id == kpz_id)
            .map(|k| decode_groups(&k.grups))
            .unwrap_or_default()
            .into_iter()
            .collect();
        self.groups
            .iter()
            .filter(|g| enabled.contains(&g.id))
            .cloned()
            .collect()
    }

    /// Function: $name.
    fn n_mb_id_by_name(&self, name: &str) -> Option<i32> {
        self.ref_n_mb
            .iter()
            .find(|(_, v)| v.eq_ignore_ascii_case(name))
            .map(|(k, _)| *k)
    }

    /// Function: $name.
    fn dict_num(map: &HashMap<i32, String>, id: Option<i32>, default: i32) -> i32 {
        let Some(idv) = id else { return default };
        map.get(&idv)
            .and_then(|s| s.trim().parse::<i32>().ok())
            .unwrap_or(default)
    }

    /// Function: $name.
    fn build_io_conn(&self) -> Result<IoConn, String> {
        let kpz_id = self.selected_kpz.ok_or_else(|| "No KPZ selected".to_string())?;
        let kpz = self
            .kpz
            .iter()
            .find(|k| k.id == kpz_id)
            .ok_or_else(|| format!("KPZ {} not found", kpz_id))?;
        let obj = self
            .obj_rows
            .iter()
            .find(|o| o.id == kpz.obj)
            .ok_or_else(|| format!("OBJ {} not found", kpz.obj))?;

        let ip = obj
            .ip
            .and_then(|id| self.ref_ip.get(&id).cloned())
            .or_else(|| {
                obj.ip_raw
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty() && s.parse::<i32>().is_err())
                    .map(ToOwned::to_owned)
            })
            .or_else(|| {
                let name = obj.name.trim();
                if name.parse::<IpAddr>().is_ok() {
                    Some(name.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "127.0.0.1".to_string());
        let port = obj
            .port
            .and_then(|id| self.ref_port.get(&id).cloned())
            .and_then(|s| s.trim().parse::<u16>().ok())
            .unwrap_or(5100);

        Ok(IoConn {
            ip,
            port,
            rtu: kpz.rtu.max(1) as u16,
            modem: kpz.modem.unwrap_or(50002).max(0) as u16,
            kan: Self::dict_num(&self.ref_kanal, obj.kanal, 3).clamp(0, 255) as u8,
            speed: Self::dict_num(&self.ref_speed, obj.speed, 8).clamp(0, 255) as u8,
            stop: Self::dict_num(&self.ref_stop, obj.stop, 0).clamp(0, 255) as u8,
            par: Self::dict_num(&self.ref_parit, obj.parit, 2).clamp(0, 255) as u8,
            data: Self::dict_num(&self.ref_bit, obj.bit, 8).clamp(0, 255) as u8,
            max_pkt_len: kpz.max_pkt_len.unwrap_or(800).max(256) as usize,
        })
    }

    /// Function: $name.
    fn as_service_conn(conn: &IoConn) -> modbus_service::ServiceConn {
        modbus_service::ServiceConn {
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
        }
    }

    #[allow(dead_code)]
    /// Function: $name.
    fn read_reg_direct(conn: &IoConn, row: &KpzIoRow, func: u8) -> Result<(f64, Vec<u8>, Vec<u8>), String> {
        let words_cnt: u16 = if matches!(row.tip, 2 | 4 | 5) { 2 } else { 1 };
        let mb = modbus::sout_mb_only(conn.rtu, func, row.mb, words_cnt, None)?;
        let (tx, resp_res) = send_mb_over_udp(conn, &mb, Duration::from_millis(IO_REQ_TIMEOUT_MS));
        let resp = match resp_res {
            Ok(v) => v,
            Err(e) => return Err(format!("{e}; tx={}", hex_join(&tx))),
        };
        let words = parse_read_words_from_resp(&resp, func)?;
        if words.is_empty() {
            return Err("empty words".to_string());
        }
        if matches!(row.tip, 2 | 4 | 5) {
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
            let v = match row.tip {
                5 => f32::from_be_bytes(bytes) as f64,
                4 => u32::from_be_bytes(bytes) as f64,
                2 => i32::from_be_bytes(bytes) as f64,
                _ => u32::from_be_bytes(bytes) as f64,
            };
            return Ok((v, tx, resp));
        }
        if row.tip == 0 {
            if let Some(bit) = row.bits {
                if bit < 0 || bit > 15 {
                    return Err(format!("bad bit index {}", bit));
                }
                let w = words[0];
                return Ok((((w >> bit) & 1) as f64, tx, resp));
            }
        }
        let w = words[0];
        let v = match row.tip {
            1 => (w as i16) as f64,
            _ => w as f64,
        };
        Ok((v, tx, resp))
    }

    /// Function: $name.
    fn write_reg_direct(conn: &IoConn, row: &KpzIoRow, value: f64) -> Result<(Vec<u8>, Vec<u8>), String> {
        let words: Vec<u16> = if matches!(row.tip, 2 | 4 | 5) {
            let bytes = match row.tip {
                5 => (value as f32).to_be_bytes().to_vec(),
                4 => (value.max(0.0) as u32).to_be_bytes().to_vec(),
                2 => (value as i32).to_be_bytes().to_vec(),
                _ => (value as i32).to_be_bytes().to_vec(),
            };
            vec![
                (((bytes[0] as u16) << 8) | bytes[1] as u16),
                (((bytes[2] as u16) << 8) | bytes[3] as u16),
            ]
        } else {
            let w = if row.tip == 1 {
                (value as i16) as u16
            } else {
                value.max(0.0) as u16
            };
            vec![w]
        };
        let mut dat: Vec<u8> = Vec::with_capacity(words.len() * 2);
        for w in &words {
            dat.push(((w >> 8) & 0xFF) as u8);
            dat.push((w & 0xFF) as u8);
        }
        let mb = modbus::sout_mb_only(conn.rtu, 16, row.mb, words.len() as u16, Some(&dat))?;
        let (tx, resp_res) = send_mb_over_udp(conn, &mb, Duration::from_millis(IO_REQ_TIMEOUT_MS));
        let resp = match resp_res {
            Ok(v) => v,
            Err(e) => return Err(format!("{e}; tx={}", hex_join(&tx))),
        };
        Ok((tx, resp))
    }

}

impl eframe::App for Ss5App {
    /// Function: $name.
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        // Keep UI ticking even without user input so periodic log refresh works.
        ctx.request_repaint_after(self.auto_refresh_every);

        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(egui::Color32::from_rgb(205, 248, 234));
        visuals.panel_fill = egui::Color32::from_rgb(6, 10, 16);
        visuals.window_fill = egui::Color32::from_rgb(9, 15, 24);
        visuals.extreme_bg_color = egui::Color32::from_rgb(4, 8, 13);
        visuals.faint_bg_color = egui::Color32::from_rgb(13, 21, 34);
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(10, 16, 26);
        visuals.widgets.noninteractive.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(34, 74, 112));
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(12, 24, 38);
        visuals.widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 102, 151));
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(18, 44, 69);
        visuals.widgets.hovered.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 188, 226));
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(10, 62, 88);
        visuals.widgets.active.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 217, 184));
        visuals.selection.bg_fill = egui::Color32::from_rgb(24, 128, 112);
        visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(66, 230, 200));
        visuals.hyperlink_color = egui::Color32::from_rgb(80, 206, 255);
        visuals.warn_fg_color = egui::Color32::from_rgb(255, 201, 94);
        visuals.error_fg_color = egui::Color32::from_rgb(255, 111, 122);
        ctx.set_visuals(visuals);

        ctx.style_mut(|s| {
            s.spacing.scroll = egui::style::ScrollStyle::solid();
            s.visuals.window_shadow = egui::epaint::Shadow {
                offset: egui::vec2(0.0, 10.0),
                blur: 24.0,
                spread: 0.0,
                color: egui::Color32::from_black_alpha(160),
            };
        });
        if self.last_auto_refresh.elapsed() >= self.auto_refresh_every {
            self.reload_logs();
            self.last_auto_refresh = Instant::now();
        }

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Перезагрузить все").clicked() {
                    self.reload_all();
                }
                if ui.button("Обновить логи").clicked() {
                    self.reload_logs();
                }

                ui.separator();
                ui.label("KPZ:");

                let mut selected = self.selected_kpz;
                let selected_text = selected
                    .and_then(|id| self.kpz.iter().find(|k| k.id == id))
                    .map(|k| format!("{} - {}", k.id, k.name))
                    .unwrap_or_else(|| "<none>".to_string());
                let popup_id = ui.make_persistent_id("kpz_combo_popup");
                let trigger = ui.add_sized([220.0, 0.0], egui::Button::new(selected_text));
                if trigger.clicked() {
                    ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                }
                egui::popup::popup_below_widget(
                    ui,
                    popup_id,
                    &trigger,
                    egui::PopupCloseBehavior::CloseOnClickOutside,
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.label("РїРѕРёСЃРє:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.kpz_filter)
                                    .desired_width(180.0),
                            );
                        });
                        ui.separator();
                        let filter = self.kpz_filter.trim().to_lowercase();
                        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                            for row in &self.kpz {
                                if !filter.is_empty() {
                                    let id_match = row.id.to_string().contains(filter.as_str());
                                    let name_match =
                                        row.name.to_lowercase().contains(filter.as_str());
                                    if !id_match && !name_match {
                                        continue;
                                    }
                                }
                                let label = format!("{} - {}", row.id, row.name);
                                if ui
                                    .selectable_label(selected == Some(row.id), label)
                                    .clicked()
                                {
                                    selected = Some(row.id);
                                    ui.memory_mut(|mem| mem.close_popup());
                                }
                            }
                        });
                    },
                );

                if selected != self.selected_kpz {
                    match self.save_quick_kpz_meta_for_selected() {
                        Ok(_) => {
                            self.selected_kpz = selected;
                            self.sync_kpz_editor_from_selected();
                            self.sync_kpz_full_editor_from_selected();
                            self.reload_logs();
                        }
                        Err(e) => self.set_error(e),
                    }
                }

                ui.separator();
                ui.checkbox(&mut self.start_flag, "start");
                ui.label("t_a:");
                ui.add(egui::TextEdit::singleline(&mut self.kpz_t_a_edit).desired_width(60.0));
                ui.label("t_pre:");
                ui.add(egui::TextEdit::singleline(&mut self.kpz_t_script_edit).desired_width(60.0));
                if ui.button("Запись").clicked() {
                    match self.save_quick_kpz_meta_for_selected() {
                        Ok(saved) => {
                            if saved {
                                self.reload_logs();
                            } else {
                                self.kpz_save_status = Some("Без изменений".to_string());
                            }
                        }
                        Err(e) => self.set_error(e),
                    }
                }

                ui.separator();
                ui.menu_button("Окна", |ui| {
                    if ui.button("Редактор KPZ").clicked() {
                        self.open_kpz_editor();
                        ui.close_menu();
                    }
                    if ui.button("Редактор OBJ").clicked() {
                        self.open_obj_editor();
                        ui.close_menu();
                    }
                    if ui.button("Редактор REG").clicked() {
                        self.open_reg_editor();
                        ui.close_menu();
                    }
                    if ui.button("Редактор справочников").clicked() {
                        self.open_dict_editor();
                        ui.close_menu();
                    }
                    if ui.button("Редактор UI-связей").clicked() {
                        self.open_ui_link_editor();
                        ui.close_menu();
                    }
                    if ui.button("Тревоги").clicked() {
                        self.open_alarm_window();
                        ui.close_menu();
                    }
                    if ui.button("Группы").clicked() {
                        self.open_group_editor();
                        ui.close_menu();
                    }
                    if ui.button("GScript").clicked() {
                        self.open_gscript_editor();
                        ui.close_menu();
                    }
                    if ui.button("График ARX").clicked() {
                        self.open_graph_window();
                        ui.close_menu();
                    }
                    if ui.button("Ввод/вывод KPZ").clicked() {
                        self.open_kpz_io_window();
                        ui.close_menu();
                    }
                    if ui.button("Диапазон KPZ").clicked() {
                        self.open_range_kpz_window();
                        ui.close_menu();
                    }
                    if ui.button("Конфигурация runtime").clicked() {
                        self.open_runtime_cfg_window();
                        ui.close_menu();
                    }
                    if ui.button("Состояние ARX").clicked() {
                        self.open_arx_state_window();
                        ui.close_menu();
                    }
                    if ui.button("Значения ARX").clicked() {
                        self.open_arx_val_window();
                        ui.close_menu();
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Журналы");
                ui.separator();
                ui.label("poll:");
                ui.add(egui::DragValue::new(&mut self.poll_log_limit).range(10..=5000));
                ui.label("elam:");
                ui.add(egui::DragValue::new(&mut self.elam_limit).range(10..=5000));
                if ui.button("Обновить").clicked() {
                    self.reload_logs();
                }
            });

            ui.horizontal(|ui| {
                ui.label("ELAM СЃ:");
                ui.add(egui::TextEdit::singleline(&mut self.elam_ts_from).desired_width(150.0));
                if ui.small_button("...").clicked() {
                    self.open_elam_time_picker(ElamTimeTarget::From);
                }
                ui.label("РїРѕ:");
                ui.add(egui::TextEdit::singleline(&mut self.elam_ts_to).desired_width(150.0));
                if ui.small_button("...").clicked() {
                    self.open_elam_time_picker(ElamTimeTarget::To);
                }
                if ui.button("Очистить").clicked() {
                    self.elam_ts_from.clear();
                    self.elam_ts_to.clear();
                    self.reload_logs();
                }
            });

            if let Some(err) = &self.load_err {
                ui.colored_label(egui::Color32::RED, err);
            }
            if let Some(status) = &self.kpz_save_status {
                ui.colored_label(egui::Color32::GREEN, status);
            }

            ui.separator();
            ui.columns(2, |cols| {
                cols[0].heading("Журнал опроса");
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(&mut cols[0], |ui| {
                        for row in &self.poll_log {
                            let kpz = row
                                .kpz_id
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string());
                            ui.label(format!("{} | kpz={} | {} | {}", row.ts, kpz, row.kind, row.msg));
                        }
                    });

                cols[1].heading("ELAM");
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(&mut cols[1], |ui| {
                        for (idx, row) in self.elam.iter().enumerate() {
                            let (expected, received) = elam_expected_received(row);
                            let func = row
                                .func
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let addr = row
                                .addr_human
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let cnt = expected
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let rx = received
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let summary = format!(
                                "{} | kpz={} | f={} a={} w={}/{} | {}",
                                row.ts, row.kpz_id, func, addr, rx, cnt, row.status
                            );
                            if ui
                                .selectable_label(self.selected_elam_index == Some(idx), summary)
                                .clicked()
                            {
                                self.selected_elam_index = Some(idx);
                                self.show_elam_details = true;
                            }
                        }
                    });
            });
        });

        if self.elam_time_picker_open {
            let mut open = self.elam_time_picker_open;
            egui::Window::new("Время ELAM")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Y");
                        ui.add(egui::DragValue::new(&mut self.elam_time_picker_year).speed(1));
                        ui.label("M");
                        ui.add(egui::DragValue::new(&mut self.elam_time_picker_month).range(1..=12));
                        ui.label("D");
                        ui.add(egui::DragValue::new(&mut self.elam_time_picker_day).range(1..=31));
                    });
                    ui.horizontal(|ui| {
                        ui.label("h");
                        ui.add(egui::DragValue::new(&mut self.elam_time_picker_hour).range(0..=23));
                        ui.label("m");
                        ui.add(egui::DragValue::new(&mut self.elam_time_picker_min).range(0..=59));
                        ui.label("s");
                        ui.add(egui::DragValue::new(&mut self.elam_time_picker_sec).range(0..=59));
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Сейчас").clicked() {
                            self.set_elam_time_picker_now();
                        }
                        if ui.button("Начало дня").clicked() {
                            self.set_elam_time_picker_day_start();
                        }
                        if ui.button("Применить").clicked() {
                            match self.apply_elam_time_picker_to_field() {
                                Ok(()) => {
                                    self.elam_time_picker_open = false;
                                    self.reload_logs();
                                }
                                Err(e) => self.set_error(e),
                            }
                        }
                    });
                });
            self.elam_time_picker_open = open;
        }

        if self.show_elam_details {
            if let Some(idx) = self.selected_elam_index {
                if let Some(row) = self.elam.get(idx).cloned() {
                    let mut open = self.show_elam_details;
                    egui::Window::new("Детали ELAM")
                        .open(&mut open)
                        .resizable(true)
                        .default_size([760.0, 520.0])
                        .show(ctx, |ui| {
                            let (expected, received) = elam_expected_received(&row);
                            ui.label(format!("id: {}", row.id));
                            ui.label(format!("ts: {}", row.ts));
                            ui.label(format!("kpz: {}", row.kpz_id));
                            ui.label(format!(
                                "group: {}",
                                row.group_id.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
                            ));
                            ui.label(format!(
                                "func: {}",
                                row.func.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
                            ));
                            ui.label(format!(
                                "addr: {}",
                                row.addr_human.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
                            ));
                            ui.label(format!(
                                "expected/received: {}/{}",
                                expected.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string()),
                                received.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
                            ));
                            ui.label(format!(
                                "duration_ms: {}",
                                row.duration_ms.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
                            ));
                            ui.label(format!("status: {}", row.status));
                            ui.separator();
                            ui.checkbox(&mut self.elam_full_hex, "full hex");
                            let mut req_text = if self.elam_full_hex {
                                hex_full(&row.req)
                            } else {
                                format_elam_packet(&row.req, false, 22)
                            };
                            let mut resp_text = match row.resp.as_deref() {
                                Some(resp) if self.elam_full_hex => hex_full(resp),
                                Some(resp) => format_elam_packet(resp, false, 22),
                                None => "<none>".to_string(),
                            };
                            ui.label("REQ:");
                            ui.add(
                                egui::TextEdit::multiline(&mut req_text)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(6)
                                    .interactive(false),
                            );
                            ui.label("RESP:");
                            ui.add(
                                egui::TextEdit::multiline(&mut resp_text)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(6)
                                    .interactive(false),
                            );
                        });
                    self.show_elam_details = open;
                } else {
                    self.show_elam_details = false;
                    self.selected_elam_index = None;
                }
            } else {
                self.show_elam_details = false;
            }
        }

        self.show_graph_window(ctx);

        self.show_kpz_io_window(ctx);

        self.show_range_kpz_window(ctx);
        self.show_runtime_cfg_window(ctx);
        self.show_arx_state_window(ctx);
        self.show_arx_val_window(ctx);
        self.show_kpz_editor(ctx);
        self.show_obj_editor(ctx);
        self.show_reg_editor(ctx);
        self.show_dict_editor(ctx);
        self.show_alarm_window(ctx);
        self.show_group_editor(ctx);
        self.show_gscript_windows(ctx);

        let kpz_name = self.selected_kpz_name();
        let actions = show_ui_link_editor(
            ctx,
            &mut self.ui_link_editor,
            self.selected_kpz,
            &kpz_name,
            &self.groups,
        );
        for a in actions {
            match a {
                UiLinkEditorAction::ReloadWindows => self.ui_link_reload_windows(),
                UiLinkEditorAction::SelectWindow(id) => self.ui_link_load_window(id),
                UiLinkEditorAction::UpsertWindow => self.ui_link_upsert_window(),
                UiLinkEditorAction::ReloadRegs => self.ui_link_reload_regs(),
                UiLinkEditorAction::SaveAll => self.ui_link_save_all(),
                UiLinkEditorAction::PollNow => self.ui_link_poll_now(),
                UiLinkEditorAction::SendTu { reg_id, on } => self.ui_link_send_tu(reg_id, on),
                UiLinkEditorAction::WriteValue { reg_id, val } => self.ui_link_write_value(reg_id, val),
            }
        }

        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

















