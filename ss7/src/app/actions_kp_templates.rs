use std::collections::BTreeSet;
use std::thread;

use crate::app::{
    IoTaskResult, LoadKpTemplateWorkerResult, ReloadKpTemplatesWorkerResult,
    ReloadKpzKpTemplateLinkWorkerResult, Ss7App,
};
use crate::db::Db;

impl Ss7App {
    pub(crate) fn open_kp_template_editor(&mut self) {
        self.ui_link_editor.open = true;
        self.ui_link_editor.kp_template_open = true;
        self.ui_link_editor.err = None;
        self.ui_link_editor.status = None;
        self.ui_link_editor.template_editor_mode = false;
        self.ui_link_editor.kp_template_editor_mode = true;
        self.ui_link_editor.kp_binding_editor_mode = false;
        self.ui_link_reload_kp_templates();
    }

    pub(crate) fn open_kp_binding_editor(&mut self) {
        self.ui_link_editor.open = true;
        self.ui_link_editor.kp_binding_open = true;
        self.ui_link_editor.err = None;
        self.ui_link_editor.status = None;
        self.ui_link_editor.template_editor_mode = false;
        self.ui_link_editor.kp_template_editor_mode = false;
        self.ui_link_editor.kp_binding_editor_mode = true;
        self.ui_link_reload_kp_templates();
    }

    pub(crate) fn ui_link_reload_kp_templates(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("loading kp templates in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| db.get_ui_kp_templates()) {
                Ok(rows) => IoTaskResult::ReloadKpTemplates(ReloadKpTemplatesWorkerResult {
                    rows,
                    status: Some("kp templates loaded".to_string()),
                    err: None,
                }),
                Err(e) => IoTaskResult::ReloadKpTemplates(ReloadKpTemplatesWorkerResult {
                    rows: Vec::new(),
                    status: None,
                    err: Some(format!("load kp templates failed: {}", crate::app::format_error_chain(&e))),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_reload_kp_template_windows(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(kp_template_id) = self.ui_link_editor.selected_kp_template_id else {
            self.ui_link_editor.kp_template_windows.clear();
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("loading kp template windows in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| db.get_ui_kp_template_windows(kp_template_id)) {
                Ok(rows) => IoTaskResult::ReloadKpTemplateWindows(crate::app_windows::ReloadKpTemplateWindowsWorkerResult {
                    rows,
                    status: Some("kp template windows loaded".to_string()),
                    err: None,
                }),
                Err(e) => IoTaskResult::ReloadKpTemplateWindows(crate::app_windows::ReloadKpTemplateWindowsWorkerResult {
                    rows: Vec::new(),
                    status: None,
                    err: Some(format!("load kp template windows failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_reload_kp_binding_template_windows(&mut self) {
        if self.io_task_rx.is_some() {
            self.pending_ui_link_kp_binding_template_windows_reload = true;
            return;
        }
        let Some(kp_template_id) = self.ui_link_editor.selected_kp_binding_template_id else {
            self.ui_link_editor.kp_binding_template_windows.clear();
            self.pending_ui_link_kp_binding_template_windows_reload = false;
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.pending_ui_link_kp_binding_template_windows_reload = false;
        self.ui_link_editor.status = Some("loading binding kp template windows in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| db.get_ui_kp_template_windows(kp_template_id)) {
                Ok(rows) => IoTaskResult::ReloadKpBindingTemplateWindows(
                    crate::app_windows::ReloadKpBindingTemplateWindowsWorkerResult {
                        rows,
                        status: Some("binding kp template windows loaded".to_string()),
                        err: None,
                    },
                ),
                Err(e) => IoTaskResult::ReloadKpBindingTemplateWindows(
                    crate::app_windows::ReloadKpBindingTemplateWindowsWorkerResult {
                        rows: Vec::new(),
                        status: None,
                        err: Some(format!("load binding kp template windows failed: {e}")),
                    },
                ),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_reload_kpz_kp_template_link(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(kpz_id) = self.selected_kpz else {
            self.ui_link_editor.kpz_kp_template_link = None;
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("loading kpz->kp template link in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| db.get_ui_kpz_kp_template_link(kpz_id)) {
                Ok(row) => IoTaskResult::ReloadKpzKpTemplateLink(ReloadKpzKpTemplateLinkWorkerResult {
                    row,
                    reload_windows: false,
                    status: Some("kpz kp-template link loaded".to_string()),
                    err: None,
                }),
                Err(e) => IoTaskResult::ReloadKpzKpTemplateLink(ReloadKpzKpTemplateLinkWorkerResult {
                    row: None,
                    reload_windows: false,
                    status: None,
                    err: Some(format!("load kpz kp-template link failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_select_kp_template(&mut self, kp_template_id: Option<i64>) {
        self.ui_link_editor.selected_kp_template_id = kp_template_id;
        if let Some(tpl) = kp_template_id.and_then(|id| self.ui_link_editor.kp_templates.iter().find(|t| t.id == id)) {
            self.ui_link_editor.kp_template_code = tpl.code.clone();
            self.ui_link_editor.kp_template_title = tpl.title.clone();
            self.ui_link_editor.kp_template_description = tpl.description.clone().unwrap_or_default();
            self.ui_link_load_kp_template(kp_template_id);
        } else {
            self.ui_link_editor.kp_template_code.clear();
            self.ui_link_editor.kp_template_title.clear();
            self.ui_link_editor.kp_template_description.clear();
            self.ui_link_editor.kp_template_windows.clear();
        }
    }

    pub(crate) fn ui_link_select_kp_binding_template(&mut self, kp_template_id: Option<i64>) {
        self.ui_link_editor.selected_kp_binding_template_id = kp_template_id;
        self.ui_link_editor.kp_binding_template_windows.clear();
        self.ui_link_reload_kp_binding_template_windows();
    }

    pub(crate) fn ui_link_save_kp_template(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let code = self.ui_link_editor.kp_template_code.trim().to_string();
        let title = self.ui_link_editor.kp_template_title.trim().to_string();
        if code.is_empty() || title.is_empty() {
            self.ui_link_editor.err = Some("kp template code/title are required".to_string());
            return;
        }
        let desc = self.ui_link_editor.kp_template_description.trim().to_string();
        let desc_bg = if desc.is_empty() { None } else { Some(desc.clone()) };
        let kp_template_id = self.ui_link_editor.selected_kp_template_id;
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("saving kp template in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| {
                db.upsert_ui_kp_template(kp_template_id, &code, &title, desc_bg.as_deref(), true)
            }) {
                Ok(kp_template_id) => IoTaskResult::LoadKpTemplate(LoadKpTemplateWorkerResult {
                    kp_template_id,
                    code,
                    title,
                    description: desc,
                    rows: Vec::new(),
                    err: None,
                }),
                Err(e) => IoTaskResult::LoadKpTemplate(LoadKpTemplateWorkerResult {
                    kp_template_id: 0,
                    code,
                    title,
                    description: desc,
                    rows: Vec::new(),
                    err: Some(format!("save kp template failed: {}", crate::app::format_error_chain(&e))),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_delete_kp_template(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(kp_template_id) = self
            .ui_link_editor
            .selected_kp_binding_template_id
            .or(self.ui_link_editor.selected_kp_template_id)
        else {
            self.ui_link_editor.err = Some("Select kp template first".to_string());
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("deleting kp template in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| db.delete_ui_kp_template(kp_template_id)) {
                Ok(()) => IoTaskResult::ReloadKpTemplates(ReloadKpTemplatesWorkerResult {
                    rows: Db::connect_from_env().and_then(|db| db.get_ui_kp_templates()).unwrap_or_default(),
                    status: Some(format!("kp template deleted: {}", kp_template_id)),
                    err: None,
                }),
                Err(e) => IoTaskResult::ReloadKpTemplates(ReloadKpTemplatesWorkerResult {
                    rows: Vec::new(),
                    status: None,
                    err: Some(format!("delete kp template failed: {e}")),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_apply_kp_template_to_kpz(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(kpz_id) = self.selected_kpz else {
            self.ui_link_editor.err = Some("Select KPZ first".to_string());
            return;
        };
        let Some(kp_template_id) = self.ui_link_editor.selected_kp_binding_template_id else {
            self.ui_link_editor.err = Some("Select kp template first".to_string());
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("applying kp template to kpz in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| {
                db.set_ui_kpz_kp_template_link(kpz_id, kp_template_id)?;
                let existing = db.get_ui_kpz_windows(kpz_id)?;
                if existing.is_empty() {
                    let rows = db.get_ui_kp_template_windows(kp_template_id)?;
                    let all_templates = db.get_ui_kpz_window_templates()?;
                    let mut existing_codes = BTreeSet::new();
                    let mut created_count = 0usize;
                    for row in rows {
                        let Some(tpl) = all_templates.iter().find(|t| t.id == row.window_template_id) else {
                            continue;
                        };
                        let code = crate::app::next_available_window_code(&mut existing_codes, &tpl.code);
                        let title = if tpl.title.trim().is_empty() { tpl.code.clone() } else { tpl.title.clone() };
                        let window_id =
                            db.upsert_ui_kpz_window(kpz_id, &code, &title, tpl.description.as_deref(), true)?;
                        db.apply_ui_kpz_window_template_to_window(tpl.id, window_id)?;
                        created_count += 1;
                    }
                    Ok(IoTaskResult::ReloadKpzKpTemplateLink(ReloadKpzKpTemplateLinkWorkerResult {
                        row: db.get_ui_kpz_kp_template_link(kpz_id)?,
                        reload_windows: true,
                        status: Some(format!(
                            "kp template {} applied to kpz {}; created windows: {}",
                            kp_template_id, kpz_id, created_count
                        )),
                        err: None,
                    }))
                } else {
                    Ok(IoTaskResult::ReloadKpzKpTemplateLink(ReloadKpzKpTemplateLinkWorkerResult {
                        row: db.get_ui_kpz_kp_template_link(kpz_id)?,
                        reload_windows: true,
                        status: Some(format!(
                            "kp template {} linked to kpz {}; windows already exist: {}",
                            kp_template_id,
                            kpz_id,
                            existing.len()
                        )),
                        err: None,
                    }))
                }
            }) {
                Ok(msg) => msg,
                Err(e) => IoTaskResult::ReloadKpzKpTemplateLink(ReloadKpzKpTemplateLinkWorkerResult {
                    row: None,
                    reload_windows: false,
                    status: None,
                    err: Some(format!("apply kp template failed: {}", crate::app::format_error_chain(&e))),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_unlink_kp_template_and_delete_windows(&mut self) {
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
        self.ui_link_editor.status =
            Some("unlinking kp template and deleting windows in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| {
                db.clear_ui_kpz_kp_template_link(kpz_id)?;
                db.delete_ui_kpz_windows_by_kpz(kpz_id)
            }) {
                Ok(deleted) => IoTaskResult::ReloadKpzKpTemplateLink(ReloadKpzKpTemplateLinkWorkerResult {
                    row: None,
                    reload_windows: true,
                    status: Some(format!(
                        "kp template unlinked from kpz {}; deleted windows: {}",
                        kpz_id, deleted
                    )),
                    err: None,
                }),
                Err(e) => IoTaskResult::ReloadKpzKpTemplateLink(ReloadKpzKpTemplateLinkWorkerResult {
                    row: None,
                    reload_windows: false,
                    status: None,
                    err: Some(format!(
                        "unlink kp template + delete windows failed: {}",
                        crate::app::format_error_chain(&e)
                    )),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_add_window_template_to_kp_template(&mut self) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(kp_template_id) = self.ui_link_editor.selected_kp_template_id else {
            self.ui_link_editor.err = Some("Select/save kp template first".to_string());
            return;
        };
        let Some(window_template_id) = self.ui_link_editor.selected_template_id else {
            self.ui_link_editor.err = Some("Select window template first".to_string());
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("adding window template to kp template in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| {
                db.add_ui_window_template_to_kp_template(kp_template_id, window_template_id)?;
                db.get_ui_kp_template_windows(kp_template_id)
            }) {
                Ok(rows) => IoTaskResult::ReloadKpTemplateWindows(
                    crate::app_windows::ReloadKpTemplateWindowsWorkerResult {
                        rows,
                        status: Some("window template added to kp template".to_string()),
                        err: None,
                    },
                ),
                Err(e) => IoTaskResult::ReloadKpTemplateWindows(
                    crate::app_windows::ReloadKpTemplateWindowsWorkerResult {
                        rows: Vec::new(),
                        status: None,
                        err: Some(format!(
                            "add window template to kp template failed: {}",
                            crate::app::format_error_chain(&e)
                        )),
                    },
                ),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_remove_window_template_from_kp_template(&mut self, window_template_id: i64) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        let Some(kp_template_id) = self.ui_link_editor.selected_kp_template_id else {
            self.ui_link_editor.err = Some("Select/save kp template first".to_string());
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("removing window template from kp template in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let msg = match Db::connect_from_env().and_then(|db| {
                db.remove_ui_window_template_from_kp_template(kp_template_id, window_template_id)?;
                db.get_ui_kp_template_windows(kp_template_id)
            }) {
                Ok(rows) => IoTaskResult::ReloadKpTemplateWindows(
                    crate::app_windows::ReloadKpTemplateWindowsWorkerResult {
                        rows,
                        status: Some("window template removed from kp template".to_string()),
                        err: None,
                    },
                ),
                Err(e) => IoTaskResult::ReloadKpTemplateWindows(
                    crate::app_windows::ReloadKpTemplateWindowsWorkerResult {
                        rows: Vec::new(),
                        status: None,
                        err: Some(format!(
                            "remove window template from kp template failed: {}",
                            crate::app::format_error_chain(&e)
                        )),
                    },
                ),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn ui_link_load_kp_template(&mut self, kp_template_id: Option<i64>) {
        if self.io_task_rx.is_some() {
            self.ui_link_editor.status = Some("io task already in progress".to_string());
            return;
        }
        self.ui_link_editor.selected_kp_template_id = kp_template_id;
        let Some(id) = kp_template_id else { return; };
        let meta = self.ui_link_editor.kp_templates.iter().find(|t| t.id == id).cloned();
        let (tx, rx) = std::sync::mpsc::channel::<IoTaskResult>();
        self.io_task_rx = Some(rx);
        self.ui_link_editor.status = Some("loading kp template in background...".to_string());
        self.ui_link_editor.err = None;
        thread::spawn(move || {
            let out = match Db::connect_from_env() {
                Ok(db) => match db.get_ui_kp_template_windows(id) {
                    Ok(rows) => {
                        let (code, title, description) = if let Some(m) = meta {
                            (m.code, m.title, m.description.unwrap_or_default())
                        } else {
                            (String::new(), String::new(), String::new())
                        };
                        LoadKpTemplateWorkerResult {
                            kp_template_id: id,
                            code,
                            title,
                            description,
                            rows,
                            err: None,
                        }
                    }
                    Err(e) => LoadKpTemplateWorkerResult {
                        kp_template_id: id,
                        code: String::new(),
                        title: String::new(),
                        description: String::new(),
                        rows: Vec::new(),
                        err: Some(format!("load kp template windows failed: {e}")),
                    },
                },
                Err(e) => LoadKpTemplateWorkerResult {
                    kp_template_id: id,
                    code: String::new(),
                    title: String::new(),
                    description: String::new(),
                    rows: Vec::new(),
                    err: Some(format!("db connect failed: {e}")),
                },
            };
            let _ = tx.send(IoTaskResult::LoadKpTemplate(out));
        });
    }
}
