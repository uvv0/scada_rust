use std::collections::HashMap;
use std::sync::mpsc::Receiver;

use anyhow::Error;
use eframe::egui;

use crate::db::Db;
use crate::models::{GScriptRow, GroupRow, KpzRow, ObjRow, WebAccountRow};
use crate::app_windows::next_available_window_code;
use crate::theme::apply_im1_visuals;
use crate::ui::accounts_window::show_accounts_window;
use crate::ui::script_editor_window::show_script_editor_window;
use crate::ui::window_link_editor::{show_ui_link_editor, UiLinkEditorAction, UiLinkEditorState};

#[allow(dead_code)]
#[path = "../../ss4/src/script.rs"]
mod script_dsl;

#[path = "app/background.rs"]
mod background;
#[path = "app/actions_io.rs"]
mod actions_io;
#[path = "app/actions_windows.rs"]
mod actions_windows;
#[path = "app/actions_templates.rs"]
mod actions_templates;
#[path = "app/actions_kp_templates.rs"]
mod actions_kp_templates;
#[path = "app/actions_accounts.rs"]
mod actions_accounts;
#[path = "app/actions_scripts.rs"]
mod actions_scripts;
#[path = "app/app_support.rs"]
mod app_support;

pub(crate) use background::{
    CmdWorkerResult, IoTaskResult, LoadKpTemplateWorkerResult, LoadTemplateWorkerResult,
    PollNowWorkerResult, ReloadKpTemplatesWorkerResult, ReloadKpzKpTemplateLinkWorkerResult,
    ReloadRefsData, ReloadRegsWorkerResult, ReloadTemplateLinksWorkerResult,
    ReloadTemplatesWorkerResult, SaveAllWorkerResult, SyncTemplateImagesWorkerResult,
    UpdateTemplateImagesWorkerResult,
};

pub(crate) const IO_REQ_TIMEOUT_MS: u64 = 1200;

fn format_error_chain(err: &Error) -> String {
    let mut parts = Vec::new();
    for cause in err.chain() {
        let text = cause.to_string();
        if !text.is_empty() && parts.last() != Some(&text) {
            parts.push(text);
        }
    }
    parts.join(" | caused by: ")
}

pub struct Ss7App {
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
    selected_kpz: Option<i32>,
    modbus_a_timeout_ms: u64,
    ui_link_editor: UiLinkEditorState,
    status: Option<String>,
    err: Option<String>,
    reload_refs_rx: Option<Receiver<Result<ReloadRefsData, String>>>,
    io_task_rx: Option<Receiver<IoTaskResult>>,
    pending_kpz_ui_refresh: bool,
    pending_ui_link_windows_reload: bool,
    pending_ui_link_kp_binding_template_windows_reload: bool,
    pub(crate) accounts_window_open: bool,
    pub(crate) web_accounts: Vec<WebAccountRow>,
    pub(crate) web_account_selected_id: Option<i64>,
    pub(crate) web_account_login: String,
    pub(crate) web_account_password: String,
    pub(crate) web_account_role: String,
    pub(crate) web_account_enabled: bool,
    pub(crate) web_account_kpz_from: String,
    pub(crate) web_account_kpz_to: String,
    pub(crate) web_account_status: Option<String>,
    pub(crate) web_account_err: Option<String>,
    pub(crate) script_editor_open: bool,
    pub(crate) script_output_open: bool,
    pub(crate) script_rows: Vec<GScriptRow>,
    pub(crate) script_selected_group: Option<i32>,
    pub(crate) script_grup_input: String,
    pub(crate) script_elam_input: String,
    pub(crate) script_max_words_input: String,
    pub(crate) script_max_k_input: String,
    pub(crate) script_pre_src: String,
    pub(crate) script_post_src: String,
    pub(crate) script_enabled: bool,
    pub(crate) script_ver_input: String,
    pub(crate) script_dry_run_output: String,
    pub(crate) script_print_log: String,
    pub(crate) script_regs_out: Vec<(i32, f64)>,
    pub(crate) script_emits_out: Vec<(f64, i32, f64)>,
    pub(crate) script_editor_tab: usize,
    pub(crate) script_output_tab: usize,
    pub(crate) script_help_open: bool,
    pub(crate) script_status: Option<String>,
    pub(crate) script_err: Option<String>,
    pub(crate) script_dirty: bool,
}

impl Ss7App {
    fn runtime_timeout_ms_from_db(db: &Db) -> u64 {
        db.get_scheduler_modbus_a_timeout_ms()
            .ok()
            .flatten()
            .and_then(|v| u64::try_from(v).ok())
            .filter(|v| *v > 0)
            .unwrap_or(IO_REQ_TIMEOUT_MS)
    }

    pub fn try_new() -> anyhow::Result<Self> {
        let db = Db::connect_from_env()?;
        let kpz = db.get_all_kpz()?;
        let groups = db.get_all_groups()?;
        let obj_rows = db.get_all_obj()?;
        let modbus_a_timeout_ms = Self::runtime_timeout_ms_from_db(&db);
        let ref_ip = Self::load_dict_map(&db, "ip");
        let ref_port = Self::load_dict_map(&db, "port");
        let ref_speed = Self::load_dict_map(&db, "speed");
        let ref_parit = Self::load_dict_map(&db, "parit");
        let ref_bit = Self::load_dict_map(&db, "bit");
        let ref_stop = Self::load_dict_map(&db, "stop");
        let ref_kanal = Self::load_dict_map(&db, "kanal");
        let ref_n_mb = Self::load_dict_map(&db, "n_mb");
        let selected_kpz = kpz.first().map(|k| k.id);
        Ok(Self {
            db,
            kpz,
            groups,
            obj_rows,
            modbus_a_timeout_ms,
            ref_ip,
            ref_port,
            ref_speed,
            ref_parit,
            ref_bit,
            ref_stop,
            ref_kanal,
            ref_n_mb,
            selected_kpz,
            ui_link_editor: UiLinkEditorState {
                io_timeout_ms: modbus_a_timeout_ms,
                ..UiLinkEditorState::default()
            },
            status: Some("ready".to_string()),
            err: None,
            reload_refs_rx: None,
            io_task_rx: None,
            pending_kpz_ui_refresh: false,
            pending_ui_link_windows_reload: false,
            pending_ui_link_kp_binding_template_windows_reload: false,
            accounts_window_open: false,
            web_accounts: Vec::new(),
            web_account_selected_id: None,
            web_account_login: String::new(),
            web_account_password: String::new(),
            web_account_role: "viewer".to_string(),
            web_account_enabled: true,
            web_account_kpz_from: String::new(),
            web_account_kpz_to: String::new(),
            web_account_status: None,
            web_account_err: None,
            script_editor_open: false,
            script_output_open: false,
            script_rows: Vec::new(),
            script_selected_group: None,
            script_grup_input: String::new(),
            script_elam_input: "0".to_string(),
            script_max_words_input: "800".to_string(),
            script_max_k_input: "2".to_string(),
            script_pre_src: String::new(),
            script_post_src: String::new(),
            script_enabled: true,
            script_ver_input: "1".to_string(),
            script_dry_run_output: String::new(),
            script_print_log: String::new(),
            script_regs_out: Vec::new(),
            script_emits_out: Vec::new(),
            script_editor_tab: 0,
            script_output_tab: 0,
            script_help_open: false,
            script_status: None,
            script_err: None,
            script_dirty: false,
        })
    }
}

impl eframe::App for Ss7App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_background_events();
        self.poll_io_task_events();
        apply_im1_visuals(ctx);
        egui::TopBottomPanel::top("ss7_top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("SS7 UI Designer");
                if ui.button("Reload refs").clicked() {
                    self.start_reload_refs_async();
                }
                if let Some(e) = &self.err {
                    ui.colored_label(egui::Color32::RED, e);
                } else if let Some(s) = &self.status {
                    ui.colored_label(egui::Color32::GREEN, s);
                }
            });
            ui.horizontal(|ui| {
                ui.label("KPZ:");
                let mut selected = self.selected_kpz;
                let text = selected
                    .and_then(|id| self.kpz.iter().find(|k| k.id == id))
                    .map(|k| format!("{} - {}", k.id, k.name))
                    .unwrap_or_else(|| "<none>".to_string());
                egui::ComboBox::from_id_salt("ss7_kpz_select")
                    .selected_text(text)
                    .show_ui(ui, |ui| {
                        for k in &self.kpz {
                            ui.selectable_value(&mut selected, Some(k.id), format!("{} - {}", k.id, k.name));
                        }
                    });
                let (ip, modem) = self.kpz_ip_modem_for(selected);
                ui.separator();
                ui.label(format!("IP: {ip}"));
                ui.label(format!("modem: {modem}"));
                if selected != self.selected_kpz {
                    self.selected_kpz = selected;
                    if self.ui_link_editor.open {
                        self.refresh_ui_link_for_selected_kpz();
                    }
                }
                if ui.button("Шаблон окна").clicked() {
                    self.open_window_template_editor();
                }
                if ui.button("Шаблон набора окон").clicked() {
                    self.open_kp_template_editor();
                }
                if ui.button("Привязка КП").clicked() {
                    self.open_kp_binding_editor();
                }
                if ui.button("Просмотр окон КП").clicked() {
                    self.open_kp_window_viewer();
                }
                if ui.button("Учетки").clicked() {
                    self.open_accounts_window();
                }
                if ui.button("Scripts").clicked() {
                    self.open_script_editor();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Separate editor app for UI windows/bindings.");
            ui.label("1) Choose KPZ in top bar.");
            ui.label("2) Open one of three editors in top bar.");
            ui.label("3) Configure templates/binding/layout as needed.");
        });

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
                UiLinkEditorAction::ReloadTemplates => self.ui_link_reload_templates(),
                UiLinkEditorAction::ReloadTemplateLinks => self.ui_link_reload_template_links(),
                UiLinkEditorAction::SelectTemplate(id) => self.ui_link_select_template(id),
                UiLinkEditorAction::LinkTemplateToKpz => self.ui_link_link_template_to_kpz(),
                UiLinkEditorAction::UnlinkTemplateFromKpz => self.ui_link_unlink_template_from_kpz(),
                UiLinkEditorAction::CreateWindowFromTemplate => self.ui_link_create_window_from_template(),
                UiLinkEditorAction::SyncTemplateImagesToWindows => self.ui_link_sync_template_images_to_windows(),
                UiLinkEditorAction::UpdateTemplateImagesInWindows => self.ui_link_update_template_images_in_windows(),
                UiLinkEditorAction::ReloadKpTemplates => self.ui_link_reload_kp_templates(),
                UiLinkEditorAction::SelectKpTemplate(id) => self.ui_link_select_kp_template(id),
                UiLinkEditorAction::SelectKpBindingTemplate(id) => self.ui_link_select_kp_binding_template(id),
                UiLinkEditorAction::SaveKpTemplate => self.ui_link_save_kp_template(),
                UiLinkEditorAction::DeleteKpTemplate => self.ui_link_delete_kp_template(),
                UiLinkEditorAction::ReloadKpTemplateWindows => self.ui_link_reload_kp_template_windows(),
                UiLinkEditorAction::AddWindowTemplateToKpTemplate => self.ui_link_add_window_template_to_kp_template(),
                UiLinkEditorAction::RemoveWindowTemplateFromKpTemplate { window_template_id } => {
                    self.ui_link_remove_window_template_from_kp_template(window_template_id)
                }
                UiLinkEditorAction::ApplyKpTemplateToKpz => self.ui_link_apply_kp_template_to_kpz(),
                UiLinkEditorAction::UnlinkKpTemplateAndDeleteWindows => {
                    self.ui_link_unlink_kp_template_and_delete_windows()
                }
                UiLinkEditorAction::ReloadKpzKpTemplateLink => self.ui_link_reload_kpz_kp_template_link(),
                UiLinkEditorAction::UpsertWindow => self.ui_link_upsert_window(),
                UiLinkEditorAction::DeleteWindow => self.ui_link_delete_window(),
                UiLinkEditorAction::ReloadRegs => self.ui_link_reload_regs(),
                UiLinkEditorAction::SaveAll => self.ui_link_save_all(),
                UiLinkEditorAction::PollNow => self.ui_link_poll_now(),
                UiLinkEditorAction::SendTu { reg_id, on } => self.ui_link_send_tu(reg_id, on),
                UiLinkEditorAction::WriteValue { reg_id, val } => self.ui_link_write_value(reg_id, val),
            }
        }
        show_accounts_window(self, ctx);
        show_script_editor_window(self, ctx);
    }
}
