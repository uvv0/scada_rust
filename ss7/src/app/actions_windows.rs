use std::collections::{BTreeMap, BTreeSet};
use std::thread;

use crate::app::{IoTaskResult, ReloadRegsWorkerResult, SaveAllWorkerResult, Ss7App};
use crate::app_windows::{
    DeleteWindowWorkerResult, LoadWindowWorkerResult, ReloadWindowsWorkerResult,
    UpsertWindowWorkerResult,
};
use crate::db::Db;
use crate::models::{AlarmRuleRow, UiWindowBindingRow, UiWindowTextItemRow};

impl Ss7App {
    pub(crate) fn refresh_ui_link_for_selected_kpz(&mut self) {
        if self.io_task_rx.is_some() {
            self.pending_kpz_ui_refresh = true;
            return;
        }
        self.pending_kpz_ui_refresh = false;
        self.ui_link_editor.selected_window_id = None;
        self.ui_link_editor.windows.clear();
        self.ui_link_editor.bindings.clear();
        self.ui_link_editor.web_safe_muted_reg_ids.clear();
        self.ui_link_editor.selected_binding_reg_id = None;
        self.ui_link_editor.selected_binding_reg_ids.clear();
        self.ui_link_editor.kpz_kp_template_link = None;
        self.ui_link_editor.kp_template_windows.clear();
        self.ui_link_editor.kp_binding_template_windows.clear();
        self.ui_link_reload_windows();
    }

    pub(crate) fn open_kp_window_viewer(&mut self) {
        self.ui_link_editor.open = true;
        self.ui_link_editor.kp_viewer_open = true;
        self.ui_link_editor.err = None;
        self.ui_link_editor.status = None;
        self.ui_link_editor.selected_window_id = None;
        self.ui_link_editor.bindings.clear();
        self.ui_link_editor.web_safe_muted_reg_ids.clear();
        self.ui_link_editor.live_values.clear();
        self.ui_link_editor.trend_history.clear();
        self.ui_link_reload_windows();
    }

    pub(crate) fn ui_link_reload_windows(&mut self) {
        if self.io_task_rx.is_some() {
            self.pending_ui_link_windows_reload = true;
            return;
        }
        let Some(kpz_id) = self.selected_kpz else {
            self.ui_link_editor.err = Some("Select KPZ first".to_string());
            self.pending_ui_link_windows_reload = false;
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.pending_ui_link_windows_reload = false;
        self.ui_link_editor.status = Some("loading windows in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| db.get_ui_kpz_windows(kpz_id)) {
                Ok(rows) => IoTaskResult::ReloadWindows(ReloadWindowsWorkerResult {
                    rows,
                    status: Some("windows loaded".to_string()),
                    err: None,
                }),
                Err(e) => IoTaskResult::ReloadWindows(ReloadWindowsWorkerResult {
                    rows: Vec::new(),
                    status: None,
                    err: Some(format!("load windows failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_upsert_window(&mut self) {
        if self.ui_link_editor.kp_template_editor_mode {
            self.ui_link_save_kp_template();
            return;
        }
        if self.ui_link_editor.template_editor_mode {
            self.ui_link_upsert_template();
            return;
        }
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let code = self.ui_link_editor.window_code.trim().to_string();
        let title = self.ui_link_editor.window_title.trim().to_string();
        if code.is_empty() || title.is_empty() {
            self.ui_link_editor.err = Some("window code/title are required".to_string());
            return;
        }
        let desc = self.ui_link_editor.window_description.trim().to_string();
        let desc_opt = if desc.is_empty() { None } else { Some(desc.as_str()) };
        let Some(kpz_id) = self.selected_kpz else {
            self.ui_link_editor.err = Some("Select KPZ first".to_string());
            return;
        };
        let selected_template_id = self.ui_link_editor.selected_template_id;
        let code_bg = code.clone();
        let title_bg = title.clone();
        let desc_bg_text = desc.to_string();
        let desc_bg = desc_opt.map(|s| s.to_string());
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("saving window in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| {
                db.upsert_ui_kpz_window(kpz_id, &code_bg, &title_bg, desc_bg.as_deref(), true)
            }) {
                Ok(window_id) => {
                    let template_warning = selected_template_id.and_then(|template_id| {
                        match Db::connect_from_env()
                            .and_then(|db| db.is_ui_kpz_window_different_from_template(window_id, template_id))
                        {
                            Ok(true) => Some(format!(
                                "warning: selected template {} differs from current window layout",
                                template_id
                            )),
                            _ => None,
                        }
                    });
                    IoTaskResult::UpsertWindow(UpsertWindowWorkerResult {
                        window_id,
                        title: title_bg,
                        code: code_bg,
                        description: desc_bg_text,
                        template_warning,
                        err: None,
                    })
                }
                Err(e) => IoTaskResult::UpsertWindow(UpsertWindowWorkerResult {
                    window_id: 0,
                    title: title_bg,
                    code: code_bg,
                    description: desc_bg_text,
                    template_warning: None,
                    err: Some(format!("save window failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_delete_window(&mut self) {
        if self.ui_link_editor.kp_template_editor_mode {
            self.ui_link_delete_kp_template();
            return;
        }
        if self.ui_link_editor.template_editor_mode {
            self.ui_link_editor.status = Some("Delete template is disabled in template mode".to_string());
            return;
        }
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(window_id) = self.ui_link_editor.selected_window_id else {
            self.ui_link_editor.err = Some("Select window first".to_string());
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("deleting window in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| db.delete_ui_kpz_window(window_id)) {
                Ok(()) => IoTaskResult::DeleteWindow(DeleteWindowWorkerResult { window_id, err: None }),
                Err(e) => IoTaskResult::DeleteWindow(DeleteWindowWorkerResult {
                    window_id,
                    err: Some(format!("delete window failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_load_window(&mut self, window_id: Option<i64>) {
        if self.ui_link_editor.template_editor_mode || self.ui_link_editor.kp_template_editor_mode {
            return;
        }
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        self.ui_link_editor.selected_window_id = window_id;
        self.ui_link_editor.groups_selected.clear();
        self.ui_link_editor.bindings.clear();
        self.ui_link_editor.web_safe_muted_reg_ids.clear();
        self.ui_link_editor.regs_available.clear();
        self.ui_link_editor.regs_group_filter = None;
        self.ui_link_editor.reg_pick_one = None;
        self.ui_link_editor.regs_selected.clear();
        self.ui_link_editor.selected_binding_reg_id = None;
        self.ui_link_editor.selected_binding_reg_ids.clear();
        self.ui_link_editor.drag_binding_reg_id = None;
        self.ui_link_editor.drag_offset = None;
        self.ui_link_editor.drag_resize_mode = false;
        self.ui_link_editor.drag_group_mode = false;
        self.ui_link_editor.drag_group_start = None;
        self.ui_link_editor.drag_group_positions.clear();
        self.ui_link_editor.drag_select_mode = false;
        self.ui_link_editor.drag_select_start = None;
        self.ui_link_editor.drag_select_additive = false;
        self.ui_link_editor.live_values.clear();
        self.ui_link_editor.trend_history.clear();
        self.ui_link_editor.alarm_rules_by_reg.clear();
        self.ui_link_editor.cmd_inputs.clear();
        self.ui_link_editor.last_cmd_result.clear();
        self.ui_link_editor.dirty = false;
        self.ui_link_editor.err = None;

        let Some(id) = window_id else { return; };
        let selected_kpz = self.selected_kpz;
        let window_meta = self.ui_link_editor.windows.iter().find(|w| w.id == id).cloned();
        let selected_template_meta = self
            .ui_link_editor
            .selected_template_id
            .and_then(|tpl_id| self.ui_link_editor.templates.iter().find(|t| t.id == tpl_id).cloned());
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("loading window in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let out = match Db::connect_from_env() {
                Ok(db) => match db.get_ui_window_bindings(id) {
                    Ok(mut bs) => {
                        let reg_ids: Vec<i32> = bs.iter().map(|b| b.reg_id).collect();
                        let mut alarm_rules_by_reg: BTreeMap<i32, Vec<AlarmRuleRow>> = BTreeMap::new();
                        if let Some(kpz_id) = selected_kpz
                            && let Ok(rules) = db.get_alarm_rules(Some(kpz_id))
                        {
                            for rule in rules {
                                if reg_ids.contains(&rule.reg_id) {
                                    alarm_rules_by_reg.entry(rule.reg_id).or_default().push(rule);
                                }
                            }
                        }
                        let mut groups_selected: BTreeSet<i32> = BTreeSet::new();
                        if let Ok(group_ids) = db.get_groups_by_reg_ids(&reg_ids) {
                            for gid in group_ids {
                                groups_selected.insert(gid);
                            }
                        }
                        if let Ok(items) = db.get_ui_window_text_items(id) {
                            let mut min_reg = bs.iter().map(|b| b.reg_id).min().unwrap_or(0);
                            for it in items {
                                let text_reg = if min_reg <= -1 { min_reg - 1 } else { -1 };
                                min_reg = text_reg;
                                let is_image = it.item_kind == "image";
                                let item_label = if is_image {
                                    it.image_path.clone().filter(|s| !s.trim().is_empty()).unwrap_or(it.text)
                                } else {
                                    it.text
                                };
                                bs.push(UiWindowBindingRow {
                                    reg_id: text_reg,
                                    is_text: true,
                                    pos: it.pos,
                                    x: it.x,
                                    y: it.y,
                                    w: it.w,
                                    h: it.h,
                                    visible: it.visible,
                                    writable: false,
                                    label_override: Some(item_label),
                                    unit: None,
                                    fmt: if is_image { Some(it.fit_mode) } else { None },
                                    scale_max: if is_image { Some(it.opacity) } else { None },
                                    component_kind: if is_image { Some("image".to_string()) } else { None },
                                    web_safe_muted: it.web_safe_muted,
                                    reg_name: if is_image { "Image".to_string() } else { "Text".to_string() },
                                    reg_mb: 0,
                                    reg_n_mb: 0,
                                    reg_tip: 0,
                                    reg_bits: None,
                                });
                            }
                        }
                        bs.sort_by_key(|b| (b.pos, b.reg_id));
                        let (window_code, window_title, window_description, template_code, template_title) =
                            if let Some(w) = window_meta {
                                let wc = w.code.clone();
                                let wt = w.title.clone();
                                let wd = w.description.unwrap_or_default();
                                let (tc, tt) = if let Some(tpl) = selected_template_meta {
                                    (tpl.code, tpl.title)
                                } else {
                                    (format!("tpl_{}", w.code), w.title)
                                };
                                (wc, wt, wd, tc, tt)
                            } else {
                                (
                                    String::new(),
                                    String::new(),
                                    String::new(),
                                    String::new(),
                                    String::new(),
                                )
                            };
                        LoadWindowWorkerResult {
                            window_id: id,
                            window_code,
                            window_title,
                            window_description,
                            template_code,
                            template_title,
                            groups_selected,
                            bindings: bs,
                            alarm_rules_by_reg,
                            err: None,
                        }
                    }
                    Err(e) => LoadWindowWorkerResult {
                        window_id: id,
                        window_code: String::new(),
                        window_title: String::new(),
                        window_description: String::new(),
                        template_code: String::new(),
                        template_title: String::new(),
                        groups_selected: BTreeSet::new(),
                        bindings: Vec::new(),
                        alarm_rules_by_reg: BTreeMap::new(),
                        err: Some(format!("load bindings failed: {e}")),
                    },
                },
                Err(e) => LoadWindowWorkerResult {
                    window_id: id,
                    window_code: String::new(),
                    window_title: String::new(),
                    window_description: String::new(),
                    template_code: String::new(),
                    template_title: String::new(),
                    groups_selected: BTreeSet::new(),
                    bindings: Vec::new(),
                    alarm_rules_by_reg: BTreeMap::new(),
                    err: Some(format!("db connect failed: {e}")),
                },
            };
            let _ = tx.send(IoTaskResult::LoadWindow(out));
        });
    }

    pub(crate) fn ui_link_reload_regs(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let ids: Vec<i32> = self.ui_link_editor.groups_selected.iter().copied().collect();
        let groups_count = self.ui_link_editor.groups_selected.len();
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("loading regs in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| db.get_regs_by_groups(&ids)) {
                Ok(rows) => IoTaskResult::ReloadRegs(ReloadRegsWorkerResult {
                    regs_available: rows,
                    status: Some(format!("loaded regs for {} groups", groups_count)),
                    err: None,
                }),
                Err(e) => IoTaskResult::ReloadRegs(ReloadRegsWorkerResult {
                    regs_available: Vec::new(),
                    status: None,
                    err: Some(format!("load regs failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_save_all(&mut self) {
        if self.ui_link_editor.kp_template_editor_mode {
            self.ui_link_editor.status = Some("Save all is not used for KP template mode".to_string());
            return;
        }
        if self.ui_link_editor.template_editor_mode {
            self.ui_link_save_template_bindings();
            return;
        }
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let mut regular_bindings: Vec<_> = self
            .ui_link_editor
            .bindings
            .iter()
            .filter(|b| !(b.is_text || b.reg_id <= 0))
            .cloned()
            .collect();
        regular_bindings.sort_by_key(|b| b.pos);
        for (i, b) in regular_bindings.iter_mut().enumerate() {
            b.pos = ((i as i32) + 1) * 10;
        }

        let mut text_items: Vec<UiWindowTextItemRow> = self
            .ui_link_editor
            .bindings
            .iter()
            .filter(|b| b.is_text || b.reg_id <= 0)
            .map(|b| {
                let is_image = b.component_kind.as_deref() == Some("image");
                let text = b
                    .label_override
                    .as_ref()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| b.reg_name.clone());
                UiWindowTextItemRow {
                pos: b.pos,
                x: b.x,
                y: b.y,
                w: b.w,
                h: b.h,
                visible: b.visible,
                text: text.clone(),
                item_kind: if is_image { "image".to_string() } else { "text".to_string() },
                image_path: if is_image { Some(text) } else { None },
                fit_mode: if is_image {
                    b.fmt.clone().unwrap_or_else(|| "contain".to_string())
                } else {
                    "contain".to_string()
                },
                opacity: if is_image { b.scale_max.unwrap_or(1.0).clamp(0.0, 1.0) } else { 1.0 },
                web_safe_muted: b.web_safe_muted,
                }
            })
            .collect();
        text_items.sort_by_key(|x| x.pos);
        for (i, x) in text_items.iter_mut().enumerate() {
            x.pos = ((i as i32) + 1) * 10;
        }

        let Some(window_id) = self.ui_link_editor.selected_window_id else {
            self.ui_link_editor.err = Some("Select/save window first".to_string());
            return;
        };
        let bindings_len = regular_bindings.len();
        let text_len = text_items.len();
        let regular_bindings_bg = regular_bindings.clone();
        let text_items_bg = text_items.clone();
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("saving in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let out = match Db::connect_from_env() {
                Ok(db) => {
                    if let Err(e) = db.save_ui_window_bindings(window_id, &regular_bindings_bg) {
                        SaveAllWorkerResult {
                            regular_bindings: Vec::new(),
                            text_items: Vec::new(),
                            status: None,
                            err: Some(format!("save bindings failed: {e}")),
                        }
                    } else if let Err(e) = db.save_ui_window_text_items(window_id, &text_items_bg) {
                        SaveAllWorkerResult {
                            regular_bindings: Vec::new(),
                            text_items: Vec::new(),
                            status: None,
                            err: Some(format!("save text items failed: {e}")),
                        }
                    } else {
                        SaveAllWorkerResult {
                            regular_bindings: regular_bindings_bg,
                            text_items: text_items_bg,
                            status: Some(format!("saved: bindings={}, text={}", bindings_len, text_len)),
                            err: None,
                        }
                    }
                }
                Err(e) => SaveAllWorkerResult {
                    regular_bindings: Vec::new(),
                    text_items: Vec::new(),
                    status: None,
                    err: Some(format!("db connect failed: {e}")),
                },
            };
            let _ = tx.send(IoTaskResult::SaveAll(out));
        });
    }
}
