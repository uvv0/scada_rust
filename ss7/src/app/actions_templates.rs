use std::collections::{BTreeMap, BTreeSet};
use std::thread;

use crate::app::{
    IoTaskResult, LoadTemplateWorkerResult, ReloadTemplateLinksWorkerResult,
    ReloadTemplatesWorkerResult, Ss7App, SyncTemplateImagesWorkerResult, UpdateTemplateImagesWorkerResult,
};
use crate::app_windows::CreateWindowFromTemplateWorkerResult;
use crate::db::Db;
use crate::models::{AlarmRuleRow, UiWindowBindingRow, UiWindowTextItemRow};

fn binding_to_text_item(b: &UiWindowBindingRow) -> UiWindowTextItemRow {
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
}

fn text_item_to_binding(it: UiWindowTextItemRow, reg_id: i32) -> UiWindowBindingRow {
    let is_image = it.item_kind == "image";
    let item_label = if is_image {
        it.image_path.clone().filter(|s| !s.trim().is_empty()).unwrap_or(it.text)
    } else {
        it.text
    };
    UiWindowBindingRow {
        reg_id,
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
    }
}

fn collect_template_text_items(bindings: &[UiWindowBindingRow]) -> Vec<UiWindowTextItemRow> {
    let mut items: Vec<_> = bindings
        .iter()
        .filter(|b| b.is_text || b.reg_id <= 0)
        .map(binding_to_text_item)
        .collect();
    items.sort_by_key(|x| x.pos);
    for (i, x) in items.iter_mut().enumerate() {
        x.pos = ((i as i32) + 1) * 10;
    }
    items
}

fn append_template_text_bindings(bindings: &mut Vec<UiWindowBindingRow>, items: Vec<UiWindowTextItemRow>) {
    let mut min_reg = bindings.iter().map(|b| b.reg_id).min().unwrap_or(0);
    for it in items {
        let text_reg = if min_reg <= -1 { min_reg - 1 } else { -1 };
        min_reg = text_reg;
        bindings.push(text_item_to_binding(it, text_reg));
    }
    bindings.sort_by_key(|b| (b.pos, b.reg_id));
}

impl Ss7App {
    pub(crate) fn open_window_template_editor(&mut self) {
        self.ui_link_editor.open = true;
        self.ui_link_editor.window_template_open = true;
        self.ui_link_editor.err = None;
        self.ui_link_editor.status = None;
        self.ui_link_editor.template_editor_mode = true;
        self.ui_link_editor.kp_template_editor_mode = false;
        self.ui_link_editor.kp_binding_editor_mode = false;
        self.ui_link_reload_templates();
    }

    pub(crate) fn ui_link_reload_templates(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("loading templates in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| db.get_ui_kpz_window_templates()) {
                Ok(rows) => IoTaskResult::ReloadTemplates(ReloadTemplatesWorkerResult {
                    rows,
                    status: Some("templates loaded".to_string()),
                    err: None,
                }),
                Err(e) => IoTaskResult::ReloadTemplates(ReloadTemplatesWorkerResult {
                    rows: Vec::new(),
                    status: None,
                    err: Some(format!("load templates failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_reload_template_links(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(kpz_id) = self.selected_kpz else {
            self.ui_link_editor.err = Some("Select KPZ first".to_string());
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("loading kpz template links in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| db.get_ui_kpz_template_links(kpz_id)) {
                Ok(rows) => IoTaskResult::ReloadTemplateLinks(ReloadTemplateLinksWorkerResult {
                    rows,
                    status: Some("kpz template links loaded".to_string()),
                    err: None,
                }),
                Err(e) => IoTaskResult::ReloadTemplateLinks(ReloadTemplateLinksWorkerResult {
                    rows: Vec::new(),
                    status: None,
                    err: Some(format!("load kpz template links failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_select_template(&mut self, template_id: Option<i64>) {
        self.ui_link_editor.selected_template_id = template_id;
        if let Some(tpl) = template_id.and_then(|id| self.ui_link_editor.templates.iter().find(|t| t.id == id)) {
            self.ui_link_editor.template_code = tpl.code.clone();
            self.ui_link_editor.template_title = tpl.title.clone();
            if self.ui_link_editor.selected_window_id.is_none() && !self.ui_link_editor.template_editor_mode {
                self.ui_link_editor.window_code = tpl.code.clone();
                self.ui_link_editor.window_title = tpl.title.clone();
                self.ui_link_editor.window_description = tpl.description.clone().unwrap_or_default();
            }
            if self.ui_link_editor.template_editor_mode {
                self.ui_link_load_template(template_id);
            }
        } else if self.ui_link_editor.template_editor_mode {
            self.ui_link_editor.bindings.clear();
            self.ui_link_editor.web_safe_muted_reg_ids.clear();
            self.ui_link_editor.window_code.clear();
            self.ui_link_editor.window_title.clear();
            self.ui_link_editor.window_description.clear();
        }
    }

    pub(crate) fn ui_link_link_template_to_kpz(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(kpz_id) = self.selected_kpz else {
            self.ui_link_editor.err = Some("Select KPZ first".to_string());
            return;
        };
        let Some(template_id) = self.ui_link_editor.selected_template_id else {
            self.ui_link_editor.err = Some("Select template first".to_string());
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("linking template to kpz in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| db.link_ui_template_to_kpz(kpz_id, template_id, false)) {
                Ok(()) => IoTaskResult::ReloadTemplateLinks(ReloadTemplateLinksWorkerResult {
                    rows: Db::connect_from_env()
                        .and_then(|db| db.get_ui_kpz_template_links(kpz_id))
                        .unwrap_or_default(),
                    status: Some(format!("template {} linked to kpz {}", template_id, kpz_id)),
                    err: None,
                }),
                Err(e) => IoTaskResult::ReloadTemplateLinks(ReloadTemplateLinksWorkerResult {
                    rows: Vec::new(),
                    status: None,
                    err: Some(format!("link template failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_unlink_template_from_kpz(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(kpz_id) = self.selected_kpz else {
            self.ui_link_editor.err = Some("Select KPZ first".to_string());
            return;
        };
        let Some(template_id) = self.ui_link_editor.selected_template_id else {
            self.ui_link_editor.err = Some("Select template first".to_string());
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("unlinking template from kpz in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| db.unlink_ui_template_from_kpz(kpz_id, template_id)) {
                Ok(()) => IoTaskResult::ReloadTemplateLinks(ReloadTemplateLinksWorkerResult {
                    rows: Db::connect_from_env()
                        .and_then(|db| db.get_ui_kpz_template_links(kpz_id))
                        .unwrap_or_default(),
                    status: Some(format!("template {} unlinked from kpz {}", template_id, kpz_id)),
                    err: None,
                }),
                Err(e) => IoTaskResult::ReloadTemplateLinks(ReloadTemplateLinksWorkerResult {
                    rows: Vec::new(),
                    status: None,
                    err: Some(format!("unlink template failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_create_window_from_template(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(kpz_id) = self.selected_kpz else {
            self.ui_link_editor.err = Some("Select KPZ first".to_string());
            return;
        };
        let Some(template_id) = self.ui_link_editor.selected_template_id else {
            self.ui_link_editor.err = Some("Select template first".to_string());
            return;
        };
        let Some(template) = self.ui_link_editor.templates.iter().find(|t| t.id == template_id).cloned() else {
            self.ui_link_editor.err = Some("Selected template metadata not loaded".to_string());
            return;
        };
        let code = self.ui_link_editor.window_code.trim().to_string();
        let title = self.ui_link_editor.window_title.trim().to_string();
        if code.is_empty() || title.is_empty() {
            self.ui_link_editor.err = Some("window code/title are required".to_string());
            return;
        }
        let desc = self.ui_link_editor.window_description.trim().to_string();
        let desc_bg = if desc.is_empty() {
            crate::app_windows::default_window_description(
                template.description.as_deref(),
                &template.title,
                &template.code,
                None,
            )
        } else {
            Some(desc.clone())
        };
        let final_desc = desc_bg.clone().unwrap_or_default();
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("creating window from template in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| {
                let window_id = db.upsert_ui_kpz_window(kpz_id, &code, &title, desc_bg.as_deref(), true)?;
                db.apply_ui_kpz_window_template_to_window(template_id, window_id)?;
                Ok(window_id)
            }) {
                Ok(window_id) => IoTaskResult::CreateWindowFromTemplate(CreateWindowFromTemplateWorkerResult {
                    template_id,
                    window_id,
                    window_code: code,
                    window_title: title,
                    window_description: final_desc,
                    err: None,
                }),
                Err(e) => IoTaskResult::CreateWindowFromTemplate(CreateWindowFromTemplateWorkerResult {
                    template_id: template.id,
                    window_id: 0,
                    window_code: code,
                    window_title: title,
                    window_description: final_desc,
                    err: Some(format!("create window from template failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_upsert_template(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let code = self.ui_link_editor.window_code.trim().to_string();
        let title = self.ui_link_editor.window_title.trim().to_string();
        if code.is_empty() || title.is_empty() {
            self.ui_link_editor.err = Some("template code/title are required".to_string());
            return;
        }
        let desc = self.ui_link_editor.window_description.trim().to_string();
        let desc_bg = if desc.is_empty() { None } else { Some(desc.clone()) };
        let template_id = self.ui_link_editor.selected_template_id;
        let code_bg = code.clone();
        let title_bg = title.clone();
        let desc_bg_text = desc.clone();
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
        let text_items = collect_template_text_items(&self.ui_link_editor.bindings);
        let regular_bindings_bg = regular_bindings.clone();
        let text_items_bg = text_items.clone();
        let groups_selected = self.ui_link_editor.groups_selected.clone();
        let alarm_rules_by_reg_bg = self.ui_link_editor.alarm_rules_by_reg.clone();
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("saving template in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| -> anyhow::Result<LoadTemplateWorkerResult> {
                let template_id =
                    db.upsert_ui_kpz_window_template(template_id, &code_bg, &title_bg, desc_bg.as_deref(), true)?;
                db.save_ui_template_bindings(template_id, &regular_bindings_bg)?;
                db.save_ui_template_text_items(template_id, &text_items_bg)?;
                let mut bindings = regular_bindings_bg.clone();
                append_template_text_bindings(&mut bindings, text_items_bg);
                Ok(LoadTemplateWorkerResult {
                    template_id,
                    template_code: code_bg,
                    template_title: title_bg,
                    template_description: desc_bg_text,
                    groups_selected,
                    bindings,
                    alarm_rules_by_reg: alarm_rules_by_reg_bg.clone(),
                    err: None,
                })
            }) {
                Ok(res) => IoTaskResult::LoadTemplate(res),
                Err(e) => IoTaskResult::LoadTemplate(LoadTemplateWorkerResult {
                    template_id: 0,
                    template_code: code,
                    template_title: title,
                    template_description: desc,
                    groups_selected: BTreeSet::new(),
                    bindings: Vec::new(),
                    alarm_rules_by_reg: alarm_rules_by_reg_bg,
                    err: Some(format!("save template failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_load_template(&mut self, template_id: Option<i64>) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        self.ui_link_editor.selected_template_id = template_id;
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

        let Some(id) = template_id else { return; };
        let selected_kpz = self.selected_kpz;
        let template_meta = self.ui_link_editor.templates.iter().find(|t| t.id == id).cloned();
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("loading template in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let out = match Db::connect_from_env() {
                Ok(db) => match db.get_ui_template_bindings(id) {
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
                        if let Ok(items) = db.get_ui_template_text_items(id) {
                            append_template_text_bindings(&mut bs, items);
                        }
                        let (code, title, description) = if let Some(t) = template_meta {
                            (t.code, t.title, t.description.unwrap_or_default())
                        } else {
                            (String::new(), String::new(), String::new())
                        };
                        LoadTemplateWorkerResult {
                            template_id: id,
                            template_code: code,
                            template_title: title,
                            template_description: description,
                            groups_selected,
                            bindings: bs,
                            alarm_rules_by_reg,
                            err: None,
                        }
                    }
                    Err(e) => LoadTemplateWorkerResult {
                        template_id: id,
                        template_code: String::new(),
                        template_title: String::new(),
                        template_description: String::new(),
                        groups_selected: BTreeSet::new(),
                        bindings: Vec::new(),
                        alarm_rules_by_reg: BTreeMap::new(),
                        err: Some(format!("load template bindings failed: {e}")),
                    },
                },
                Err(e) => LoadTemplateWorkerResult {
                    template_id: id,
                    template_code: String::new(),
                    template_title: String::new(),
                    template_description: String::new(),
                    groups_selected: BTreeSet::new(),
                    bindings: Vec::new(),
                    alarm_rules_by_reg: BTreeMap::new(),
                    err: Some(format!("db connect failed: {e}")),
                },
            };
            let _ = tx.send(IoTaskResult::LoadTemplate(out));
        });
    }

    pub(crate) fn ui_link_save_template_bindings(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(template_id) = self.ui_link_editor.selected_template_id else {
            self.ui_link_editor.err = Some("Select/save template first".to_string());
            return;
        };
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
        let text_items = collect_template_text_items(&self.ui_link_editor.bindings);
        let regular_bindings_bg = regular_bindings.clone();
        let text_items_bg = text_items.clone();
        let alarm_rules_by_reg_bg = self.ui_link_editor.alarm_rules_by_reg.clone();
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("saving template bindings in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let out = match Db::connect_from_env() {
                Ok(db) => {
                    if let Err(e) = db.save_ui_template_bindings(template_id, &regular_bindings_bg) {
                        LoadTemplateWorkerResult {
                            template_id,
                            template_code: String::new(),
                            template_title: String::new(),
                            template_description: String::new(),
                            groups_selected: BTreeSet::new(),
                            bindings: Vec::new(),
                            alarm_rules_by_reg: alarm_rules_by_reg_bg.clone(),
                            err: Some(format!("save template bindings failed: {e}")),
                        }
                    } else if let Err(e) = db.save_ui_template_text_items(template_id, &text_items_bg) {
                        LoadTemplateWorkerResult {
                            template_id,
                            template_code: String::new(),
                            template_title: String::new(),
                            template_description: String::new(),
                            groups_selected: BTreeSet::new(),
                            bindings: Vec::new(),
                            alarm_rules_by_reg: alarm_rules_by_reg_bg.clone(),
                            err: Some(format!("save template text/image items failed: {e}")),
                        }
                    } else {
                        let mut bindings = regular_bindings_bg.clone();
                        append_template_text_bindings(&mut bindings, text_items_bg);
                        LoadTemplateWorkerResult {
                            template_id,
                            template_code: String::new(),
                            template_title: String::new(),
                            template_description: String::new(),
                            groups_selected: BTreeSet::new(),
                            bindings,
                            alarm_rules_by_reg: alarm_rules_by_reg_bg.clone(),
                            err: None,
                        }
                    }
                }
                Err(e) => LoadTemplateWorkerResult {
                    template_id,
                    template_code: String::new(),
                    template_title: String::new(),
                    template_description: String::new(),
                    groups_selected: BTreeSet::new(),
                    bindings: Vec::new(),
                    alarm_rules_by_reg: alarm_rules_by_reg_bg,
                    err: Some(format!("db connect failed: {e}")),
                },
            };
            let _ = tx.send(IoTaskResult::LoadTemplate(out));
        });
    }

    pub(crate) fn ui_link_sync_template_images_to_windows(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(template_id) = self.ui_link_editor.selected_template_id else {
            self.ui_link_editor.err = Some("Select/save template first".to_string());
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("syncing template images to matching windows...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let res = match Db::connect_from_env()
                .and_then(|db| db.sync_template_images_to_matching_windows(template_id))
            {
                Ok((items, windows)) => SyncTemplateImagesWorkerResult {
                    status: Some(format!("synced template images: {items} item(s) into {windows} window(s)")),
                    err: None,
                },
                Err(e) => SyncTemplateImagesWorkerResult {
                    status: None,
                    err: Some(format!("sync template images failed: {e}")),
                },
            };
            let _ = tx.send(IoTaskResult::SyncTemplateImages(res));
        });
    }

    pub(crate) fn ui_link_update_template_images_in_windows(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(template_id) = self.ui_link_editor.selected_template_id else {
            self.ui_link_editor.err = Some("Select/save template first".to_string());
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("updating template images in matching windows...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let res = match Db::connect_from_env()
                .and_then(|db| db.update_template_images_in_matching_windows(template_id))
            {
                Ok((items, windows)) => UpdateTemplateImagesWorkerResult {
                    status: Some(format!("updated template images: {items} item(s) in {windows} window(s)")),
                    err: None,
                },
                Err(e) => UpdateTemplateImagesWorkerResult {
                    status: None,
                    err: Some(format!("update template images failed: {e}")),
                },
            };
            let _ = tx.send(IoTaskResult::UpdateTemplateImages(res));
        });
    }
}
