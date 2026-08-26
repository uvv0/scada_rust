use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::mpsc::TryRecvError;
use std::thread;

use crate::app::Ss7App;
use crate::app_windows::{
    CreateWindowFromTemplateWorkerResult, DeleteWindowWorkerResult, LoadWindowWorkerResult,
    ReloadKpBindingTemplateWindowsWorkerResult, ReloadKpTemplateWindowsWorkerResult,
    ReloadWindowsWorkerResult, UpsertWindowWorkerResult,
};
use crate::db::Db;
use crate::models::{
    AlarmRuleRow, GroupRow, KpzRow, ObjRow, RegRow, UiKpTemplateRow, UiKpTemplateWindowRow,
    UiKpzKpTemplateLinkRow, UiKpzTemplateLinkRow, UiKpzWindowRow, UiScreenTemplateRow,
    UiWindowBindingRow, UiWindowTextItemRow,
};

pub(crate) struct ReloadRefsData {
    pub(crate) kpz: Vec<KpzRow>,
    pub(crate) groups: Vec<GroupRow>,
    pub(crate) obj_rows: Vec<ObjRow>,
    pub(crate) modbus_a_timeout_ms: u64,
    pub(crate) ref_ip: HashMap<i32, String>,
    pub(crate) ref_port: HashMap<i32, String>,
    pub(crate) ref_speed: HashMap<i32, String>,
    pub(crate) ref_parit: HashMap<i32, String>,
    pub(crate) ref_bit: HashMap<i32, String>,
    pub(crate) ref_stop: HashMap<i32, String>,
    pub(crate) ref_kanal: HashMap<i32, String>,
    pub(crate) ref_n_mb: HashMap<i32, String>,
}

pub(crate) struct PollNowWorkerResult {
    pub(crate) live_values: BTreeMap<i32, Option<f64>>,
    pub(crate) poll_trace: String,
    pub(crate) status: Option<String>,
    pub(crate) err: Option<String>,
}

pub(crate) struct CmdWorkerResult {
    pub(crate) reg_id: i32,
    pub(crate) live_value: Option<f64>,
    pub(crate) last_cmd: String,
    pub(crate) status: Option<String>,
    pub(crate) err: Option<String>,
}

pub(crate) struct ReloadRegsWorkerResult {
    pub(crate) regs_available: Vec<RegRow>,
    pub(crate) status: Option<String>,
    pub(crate) err: Option<String>,
}

pub(crate) struct SaveAllWorkerResult {
    pub(crate) regular_bindings: Vec<UiWindowBindingRow>,
    pub(crate) text_items: Vec<UiWindowTextItemRow>,
    pub(crate) status: Option<String>,
    pub(crate) err: Option<String>,
}

pub(crate) struct ReloadTemplatesWorkerResult {
    pub(crate) rows: Vec<UiScreenTemplateRow>,
    pub(crate) status: Option<String>,
    pub(crate) err: Option<String>,
}

pub(crate) struct ReloadTemplateLinksWorkerResult {
    pub(crate) rows: Vec<UiKpzTemplateLinkRow>,
    pub(crate) status: Option<String>,
    pub(crate) err: Option<String>,
}

pub(crate) struct ReloadKpTemplatesWorkerResult {
    pub(crate) rows: Vec<UiKpTemplateRow>,
    pub(crate) status: Option<String>,
    pub(crate) err: Option<String>,
}

pub(crate) struct ReloadKpzKpTemplateLinkWorkerResult {
    pub(crate) row: Option<UiKpzKpTemplateLinkRow>,
    pub(crate) reload_windows: bool,
    pub(crate) status: Option<String>,
    pub(crate) err: Option<String>,
}

pub(crate) struct LoadTemplateWorkerResult {
    pub(crate) template_id: i64,
    pub(crate) template_code: String,
    pub(crate) template_title: String,
    pub(crate) template_description: String,
    pub(crate) groups_selected: BTreeSet<i32>,
    pub(crate) bindings: Vec<UiWindowBindingRow>,
    pub(crate) alarm_rules_by_reg: BTreeMap<i32, Vec<AlarmRuleRow>>,
    pub(crate) err: Option<String>,
}

pub(crate) struct LoadKpTemplateWorkerResult {
    pub(crate) kp_template_id: i64,
    pub(crate) code: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) rows: Vec<UiKpTemplateWindowRow>,
    pub(crate) err: Option<String>,
}

pub(crate) struct SyncTemplateImagesWorkerResult {
    pub(crate) status: Option<String>,
    pub(crate) err: Option<String>,
}

pub(crate) struct UpdateTemplateImagesWorkerResult {
    pub(crate) status: Option<String>,
    pub(crate) err: Option<String>,
}

pub(crate) enum IoTaskResult {
    PollNow(PollNowWorkerResult),
    SendTu(CmdWorkerResult),
    WriteValue(CmdWorkerResult),
    ReloadRegs(ReloadRegsWorkerResult),
    SaveAll(SaveAllWorkerResult),
    ReloadWindows(ReloadWindowsWorkerResult),
    ReloadTemplates(ReloadTemplatesWorkerResult),
    ReloadTemplateLinks(ReloadTemplateLinksWorkerResult),
    ReloadKpTemplates(ReloadKpTemplatesWorkerResult),
    ReloadKpTemplateWindows(ReloadKpTemplateWindowsWorkerResult),
    ReloadKpBindingTemplateWindows(ReloadKpBindingTemplateWindowsWorkerResult),
    ReloadKpzKpTemplateLink(ReloadKpzKpTemplateLinkWorkerResult),
    CreateWindowFromTemplate(CreateWindowFromTemplateWorkerResult),
    UpsertWindow(UpsertWindowWorkerResult),
    DeleteWindow(DeleteWindowWorkerResult),
    LoadWindow(LoadWindowWorkerResult),
    LoadTemplate(LoadTemplateWorkerResult),
    LoadKpTemplate(LoadKpTemplateWorkerResult),
    SyncTemplateImages(SyncTemplateImagesWorkerResult),
    UpdateTemplateImages(UpdateTemplateImagesWorkerResult),
}

impl Ss7App {
    pub(crate) fn start_reload_refs_async(&mut self) {
        if self.reload_refs_rx.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<Result<ReloadRefsData, String>>();
        self.reload_refs_rx = Some(rx);
        self.status = Some("reload refs in progress...".to_string());
        self.err = None;
        thread::spawn(move || {
            let res = (|| -> Result<ReloadRefsData, String> {
                let db = Db::connect_from_env().map_err(|e| e.to_string())?;
                let kpz = db.get_all_kpz().map_err(|e| e.to_string())?;
                let groups = db.get_all_groups().map_err(|e| e.to_string())?;
                let obj_rows = db.get_all_obj().map_err(|e| e.to_string())?;
                let modbus_a_timeout_ms = Self::runtime_timeout_ms_from_db(&db);
                let to_map = |name: &str| -> HashMap<i32, String> {
                    db.get_items(name)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|r| (r.id, r.name))
                        .collect()
                };
                Ok(ReloadRefsData {
                    kpz,
                    groups,
                    obj_rows,
                    modbus_a_timeout_ms,
                    ref_ip: to_map("ip"),
                    ref_port: to_map("port"),
                    ref_speed: to_map("speed"),
                    ref_parit: to_map("parit"),
                    ref_bit: to_map("bit"),
                    ref_stop: to_map("stop"),
                    ref_kanal: to_map("kanal"),
                    ref_n_mb: to_map("n_mb"),
                })
            })();
            let _ = tx.send(res);
        });
    }

    pub(crate) fn poll_background_events(&mut self) {
        let Some(rx) = self.reload_refs_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(v)) => {
                self.kpz = v.kpz;
                self.groups = v.groups;
                self.obj_rows = v.obj_rows;
                self.modbus_a_timeout_ms = v.modbus_a_timeout_ms;
                self.ui_link_editor.io_timeout_ms = v.modbus_a_timeout_ms;
                self.ref_ip = v.ref_ip;
                self.ref_port = v.ref_port;
                self.ref_speed = v.ref_speed;
                self.ref_parit = v.ref_parit;
                self.ref_bit = v.ref_bit;
                self.ref_stop = v.ref_stop;
                self.ref_kanal = v.ref_kanal;
                self.ref_n_mb = v.ref_n_mb;
                self.status = Some("refs reloaded".to_string());
                self.err = None;
            }
            Ok(Err(e)) => {
                self.err = Some(format!("reload refs failed: {e}"));
            }
            Err(TryRecvError::Empty) => {
                self.reload_refs_rx = Some(rx);
            }
            Err(TryRecvError::Disconnected) => {
                self.err = Some("reload refs worker disconnected".to_string());
            }
        }
    }

    pub(crate) fn poll_io_task_events(&mut self) {
        let Some(rx) = self.io_task_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(IoTaskResult::PollNow(res)) => {
                self.ui_link_editor.live_values = res.live_values;
                let samples: Vec<(i32, f64)> = self
                    .ui_link_editor
                    .live_values
                    .iter()
                    .filter_map(|(&reg_id, value)| value.map(|v| (reg_id, v)))
                    .collect();
                for (reg_id, v) in samples {
                    crate::ui::window_link_editor::push_trend_sample(&mut self.ui_link_editor, reg_id, v);
                }
                self.ui_link_editor.poll_trace = res.poll_trace;
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
            }
            Ok(IoTaskResult::SendTu(res)) => {
                self.ui_link_editor
                    .last_cmd_result
                    .insert(res.reg_id, res.last_cmd);
                if let Some(v) = res.live_value {
                    self.ui_link_editor.live_values.insert(res.reg_id, Some(v));
                    crate::ui::window_link_editor::push_trend_sample(&mut self.ui_link_editor, res.reg_id, v);
                }
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
            }
            Ok(IoTaskResult::WriteValue(res)) => {
                self.ui_link_editor
                    .last_cmd_result
                    .insert(res.reg_id, res.last_cmd);
                if let Some(v) = res.live_value {
                    self.ui_link_editor.live_values.insert(res.reg_id, Some(v));
                    crate::ui::window_link_editor::push_trend_sample(&mut self.ui_link_editor, res.reg_id, v);
                }
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
            }
            Ok(IoTaskResult::ReloadRegs(res)) => {
                self.ui_link_editor.regs_available = res.regs_available;
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
            }
            Ok(IoTaskResult::SaveAll(res)) => {
                if res.err.is_none() {
                    self.ui_link_editor.bindings = res.regular_bindings;
                    let mut min_reg = self
                        .ui_link_editor
                        .bindings
                        .iter()
                        .map(|b| b.reg_id)
                        .min()
                        .unwrap_or(0);
                    for it in res.text_items {
                        let text_reg = if min_reg <= -1 { min_reg - 1 } else { -1 };
                        min_reg = text_reg;
                        self.ui_link_editor.bindings.push(UiWindowBindingRow {
                            reg_id: text_reg,
                            is_text: true,
                            pos: it.pos,
                            x: it.x,
                            y: it.y,
                            w: it.w,
                            h: it.h,
                            visible: it.visible,
                            writable: false,
                            label_override: Some(it.text),
                            unit: None,
                            fmt: None,
                            scale_max: None,
                            component_kind: None,
                            web_safe_muted: it.web_safe_muted,
                            reg_name: "Text".to_string(),
                            reg_mb: 0,
                            reg_n_mb: 0,
                            reg_tip: 0,
                            reg_bits: None,
                        });
                    }
                    self.ui_link_editor.bindings.sort_by_key(|b| (b.pos, b.reg_id));
                    self.ui_link_editor.sync_web_safe_muted_from_bindings();
                    self.ui_link_editor.dirty = false;
                }
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
            }
            Ok(IoTaskResult::ReloadWindows(res)) => {
                if res.err.is_none() {
                    self.ui_link_editor.windows = res.rows;
                    if self.ui_link_editor.kp_viewer_open
                        && self.ui_link_editor.selected_window_id.is_none()
                        && let Some(first) = self.ui_link_editor.windows.first()
                    {
                        self.ui_link_load_window(Some(first.id));
                    }
                    if let Some(id) = self.ui_link_editor.selected_window_id
                        && !self.ui_link_editor.windows.iter().any(|w| w.id == id)
                    {
                        self.ui_link_load_window(None);
                    }
                    if self.ui_link_editor.open {
                        self.ui_link_reload_templates();
                    }
                    if self.ui_link_editor.kp_binding_open {
                        self.ui_link_reload_kpz_kp_template_link();
                    }
                }
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
            }
            Ok(IoTaskResult::ReloadTemplates(res)) => {
                if res.err.is_none() {
                    self.ui_link_editor.templates = res.rows;
                    if let Some(id) = self.ui_link_editor.selected_template_id
                        && !self.ui_link_editor.templates.iter().any(|t| t.id == id)
                    {
                        self.ui_link_editor.selected_template_id = None;
                    }
                    if self.ui_link_editor.open && !self.ui_link_editor.kp_template_editor_mode {
                        self.ui_link_reload_template_links();
                    }
                }
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
            }
            Ok(IoTaskResult::ReloadTemplateLinks(res)) => {
                if res.err.is_none() {
                    self.ui_link_editor.linked_templates = res.rows;
                    if self.ui_link_editor.open && self.ui_link_editor.kp_binding_open {
                        self.ui_link_reload_kp_templates();
                    }
                }
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
            }
            Ok(IoTaskResult::ReloadKpTemplates(res)) => {
                if res.err.is_none() {
                    self.ui_link_editor.kp_templates = res.rows;
                    if let Some(id) = self.ui_link_editor.selected_kp_template_id
                        && !self.ui_link_editor.kp_templates.iter().any(|t| t.id == id)
                    {
                        self.ui_link_editor.selected_kp_template_id = None;
                    }
                    if let Some(id) = self.ui_link_editor.selected_kp_binding_template_id
                        && !self.ui_link_editor.kp_templates.iter().any(|t| t.id == id)
                    {
                        self.ui_link_editor.selected_kp_binding_template_id = None;
                    }
                    if self.ui_link_editor.kp_binding_open {
                        self.ui_link_reload_kpz_kp_template_link();
                    }
                }
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
            }
            Ok(IoTaskResult::ReloadKpTemplateWindows(res)) => {
                if res.err.is_none() {
                    self.ui_link_editor.kp_template_windows = res.rows;
                }
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
            }
            Ok(IoTaskResult::ReloadKpBindingTemplateWindows(res)) => {
                if res.err.is_none() {
                    self.ui_link_editor.kp_binding_template_windows = res.rows;
                }
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
            }
            Ok(IoTaskResult::ReloadKpzKpTemplateLink(res)) => {
                if res.err.is_none() {
                    self.ui_link_editor.selected_kp_binding_template_id =
                        res.row.as_ref().map(|row| row.kp_template_id);
                    self.ui_link_editor.kpz_kp_template_link = res.row;
                    if self.ui_link_editor.selected_kp_binding_template_id.is_none() {
                        self.ui_link_editor.kp_binding_template_windows.clear();
                        self.pending_ui_link_kp_binding_template_windows_reload = false;
                    } else if self.ui_link_editor.kp_binding_open {
                        if res.reload_windows {
                            self.pending_ui_link_windows_reload = true;
                        }
                        self.ui_link_reload_kp_binding_template_windows();
                    }
                    if res.reload_windows && !self.ui_link_editor.kp_binding_open {
                        self.ui_link_reload_windows();
                    }
                }
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
            }
            Ok(IoTaskResult::CreateWindowFromTemplate(res)) => {
                if let Some(err) = res.err {
                    self.ui_link_editor.err = Some(err);
                } else {
                    self.ui_link_editor.template_editor_mode = false;
                    self.ui_link_editor.selected_template_id = Some(res.template_id);
                    self.ui_link_editor.selected_window_id = Some(res.window_id);
                    self.ui_link_editor.window_code = res.window_code;
                    self.ui_link_editor.window_title = res.window_title;
                    self.ui_link_editor.window_description = res.window_description;
                    if !self.ui_link_editor.windows.iter().any(|w| w.id == res.window_id) {
                        self.ui_link_editor.windows.push(UiKpzWindowRow {
                            id: res.window_id,
                            kpz_id: self.selected_kpz.unwrap_or_default(),
                            code: self.ui_link_editor.window_code.clone(),
                            title: self.ui_link_editor.window_title.clone(),
                            description: if self.ui_link_editor.window_description.trim().is_empty() {
                                None
                            } else {
                                Some(self.ui_link_editor.window_description.clone())
                            },
                            is_active: true,
                        });
                        self.ui_link_editor
                            .windows
                            .sort_by(|a, b| a.title.cmp(&b.title).then(a.code.cmp(&b.code)));
                    }
                    self.ui_link_editor.status = Some(format!(
                        "window created from template {} -> {}",
                        res.template_id, res.window_id
                    ));
                    self.ui_link_editor.err = None;
                    self.ui_link_load_window(Some(res.window_id));
                }
            }
            Ok(IoTaskResult::UpsertWindow(res)) => {
                if let Some(err) = res.err {
                    self.ui_link_editor.err = Some(err);
                } else {
                    self.ui_link_editor.selected_window_id = Some(res.window_id);
                    let mut msg = format!("window saved: {} [{}]", res.title, res.code);
                    if let Some(w) = res.template_warning {
                        msg = format!("{msg}; {w}");
                    }
                    if let Some(existing) = self.ui_link_editor.windows.iter_mut().find(|w| w.id == res.window_id) {
                        existing.code = res.code.clone();
                        existing.title = res.title.clone();
                        existing.description = if res.description.trim().is_empty() {
                            None
                        } else {
                            Some(res.description.clone())
                        };
                    } else {
                        self.ui_link_editor.windows.push(UiKpzWindowRow {
                            id: res.window_id,
                            kpz_id: self.selected_kpz.unwrap_or_default(),
                            code: res.code.clone(),
                            title: res.title.clone(),
                            description: if res.description.trim().is_empty() {
                                None
                            } else {
                                Some(res.description.clone())
                            },
                            is_active: true,
                        });
                        self.ui_link_editor
                            .windows
                            .sort_by(|a, b| a.title.cmp(&b.title).then(a.code.cmp(&b.code)));
                    }
                    self.ui_link_editor.status = Some(msg);
                    self.ui_link_editor.err = None;
                    self.ui_link_load_window(Some(res.window_id));
                }
            }
            Ok(IoTaskResult::DeleteWindow(res)) => {
                if let Some(err) = res.err {
                    self.ui_link_editor.err = Some(err);
                } else {
                    self.ui_link_editor.clear_for_new_window();
                    self.ui_link_editor.status = Some(format!("window deleted: {}", res.window_id));
                    self.ui_link_editor.err = None;
                    self.ui_link_reload_windows();
                }
            }
            Ok(IoTaskResult::LoadWindow(res)) => {
                if let Some(err) = res.err {
                    self.ui_link_editor.err = Some(err);
                } else {
                    self.ui_link_editor.selected_window_id = Some(res.window_id);
                    self.ui_link_editor.window_code = res.window_code;
                    self.ui_link_editor.window_title = res.window_title;
                    self.ui_link_editor.window_description = res.window_description;
                    self.ui_link_editor.template_code = res.template_code;
                    self.ui_link_editor.template_title = res.template_title;
                    self.ui_link_editor.groups_selected = res.groups_selected;
                    self.ui_link_editor.selected_binding_reg_id = res.bindings.first().map(|b| b.reg_id);
                    self.ui_link_editor.alarm_rules_by_reg = res.alarm_rules_by_reg;
                    self.ui_link_editor.bindings = res.bindings;
                    self.ui_link_editor.sync_web_safe_muted_from_bindings();
                    self.ui_link_editor.err = None;
                    self.ui_link_reload_regs();
                }
            }
            Ok(IoTaskResult::LoadTemplate(res)) => {
                if let Some(err) = res.err {
                    self.ui_link_editor.err = Some(err);
                } else {
                    self.ui_link_editor.selected_template_id = Some(res.template_id);
                    if res.template_id != 0 && (!res.template_code.is_empty() || !res.template_title.is_empty()) {
                        if let Some(existing) = self
                            .ui_link_editor
                            .templates
                            .iter_mut()
                            .find(|t| t.id == res.template_id)
                        {
                            if !res.template_code.is_empty() {
                                existing.code = res.template_code.clone();
                            }
                            if !res.template_title.is_empty() {
                                existing.title = res.template_title.clone();
                            }
                            existing.description = if res.template_description.trim().is_empty() {
                                None
                            } else {
                                Some(res.template_description.clone())
                            };
                        } else {
                            self.ui_link_editor.templates.push(UiScreenTemplateRow {
                                id: res.template_id,
                                code: res.template_code.clone(),
                                title: res.template_title.clone(),
                                description: if res.template_description.trim().is_empty() {
                                    None
                                } else {
                                    Some(res.template_description.clone())
                                },
                                is_active: true,
                            });
                            self.ui_link_editor
                                .templates
                                .sort_by(|a, b| a.title.cmp(&b.title).then(a.code.cmp(&b.code)));
                        }
                    }
                    if !res.template_code.is_empty() || self.ui_link_editor.window_code.is_empty() {
                        self.ui_link_editor.window_code = res.template_code.clone();
                        self.ui_link_editor.template_code = res.template_code;
                    }
                    if !res.template_title.is_empty() || self.ui_link_editor.window_title.is_empty() {
                        self.ui_link_editor.window_title = res.template_title.clone();
                        self.ui_link_editor.template_title = res.template_title;
                    }
                    if !res.template_description.is_empty() || self.ui_link_editor.window_description.is_empty() {
                        self.ui_link_editor.window_description = res.template_description;
                    }
                    if !res.groups_selected.is_empty() {
                        self.ui_link_editor.groups_selected = res.groups_selected;
                    }
                    let layout_item_count = res.bindings.iter().filter(|b| b.is_text || b.reg_id <= 0).count();
                    let image_item_count = res
                        .bindings
                        .iter()
                        .filter(|b| b.component_kind.as_deref() == Some("image"))
                        .count();
                    if !res.bindings.is_empty() {
                        self.ui_link_editor.selected_binding_reg_id = res.bindings.first().map(|b| b.reg_id);
                        self.ui_link_editor.bindings = res.bindings;
                        self.ui_link_editor.sync_web_safe_muted_from_bindings();
                        self.ui_link_editor.dirty = false;
                    }
                    self.ui_link_editor.alarm_rules_by_reg = res.alarm_rules_by_reg;
                    self.ui_link_editor.err = None;
                    self.ui_link_editor.status = Some(format!(
                        "template ready: {} (layout items: {}, images: {})",
                        res.template_id, layout_item_count, image_item_count
                    ));
                    self.ui_link_reload_regs();
                }
            }
            Ok(IoTaskResult::LoadKpTemplate(res)) => {
                if let Some(err) = res.err {
                    self.ui_link_editor.err = Some(err);
                } else {
                    self.ui_link_editor.selected_kp_template_id = Some(res.kp_template_id);
                    if let Some(existing) = self
                        .ui_link_editor
                        .kp_templates
                        .iter_mut()
                        .find(|t| t.id == res.kp_template_id)
                    {
                        existing.code = res.code.clone();
                        existing.title = res.title.clone();
                        existing.description = if res.description.trim().is_empty() {
                            None
                        } else {
                            Some(res.description.clone())
                        };
                    } else if res.kp_template_id != 0 {
                        self.ui_link_editor.kp_templates.push(UiKpTemplateRow {
                            id: res.kp_template_id,
                            code: res.code.clone(),
                            title: res.title.clone(),
                            description: if res.description.trim().is_empty() {
                                None
                            } else {
                                Some(res.description.clone())
                            },
                            is_active: true,
                        });
                        self.ui_link_editor
                            .kp_templates
                            .sort_by(|a, b| a.title.cmp(&b.title).then(a.code.cmp(&b.code)));
                    }
                    self.ui_link_editor.kp_template_code = res.code;
                    self.ui_link_editor.kp_template_title = res.title;
                    self.ui_link_editor.kp_template_description = res.description;
                    self.ui_link_editor.kp_template_windows = res.rows;
                    self.ui_link_editor.status = Some(format!("kp template ready: {}", res.kp_template_id));
                    self.ui_link_editor.err = None;
                }
            }
            Ok(IoTaskResult::SyncTemplateImages(res)) => {
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
                if self.ui_link_editor.open {
                    self.ui_link_reload_windows();
                }
            }
            Ok(IoTaskResult::UpdateTemplateImages(res)) => {
                self.ui_link_editor.status = res.status;
                self.ui_link_editor.err = res.err;
                if self.ui_link_editor.open {
                    self.ui_link_reload_windows();
                }
            }
            Err(TryRecvError::Empty) => {
                self.io_task_rx = Some(rx);
            }
            Err(TryRecvError::Disconnected) => {
                self.ui_link_editor.err = Some("io task worker disconnected".to_string());
            }
        }
        if self.io_task_rx.is_none() {
            if self.pending_ui_link_kp_binding_template_windows_reload {
                self.pending_ui_link_kp_binding_template_windows_reload = false;
                self.ui_link_reload_kp_binding_template_windows();
            } else if self.pending_ui_link_windows_reload {
                self.pending_ui_link_windows_reload = false;
                self.ui_link_reload_windows();
            } else if self.pending_kpz_ui_refresh {
                self.refresh_ui_link_for_selected_kpz();
            }
        }
    }
}
