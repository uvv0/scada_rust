use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;
use image::ImageReader;

use crate::app::IO_REQ_TIMEOUT_MS;
use crate::models::{
    AlarmRuleRow, GroupRow, RegRow, UiKpTemplateRow, UiKpTemplateWindowRow, UiKpzKpTemplateLinkRow,
    UiKpzTemplateLinkRow, UiKpzWindowRow, UiScreenTemplateRow, UiWindowBindingRow,
};
use crate::ui::window_link_editor_preview::{
    alarm_color as preview_alarm_color, draw_bar_alarm_markers, draw_button_tile,
    draw_gauge_alarm_markers, draw_led_tile, draw_numeric_tile,
    draw_trend_tile,
};
use crate::ui::window_link_editor_web_safe::{
    apply_profile as apply_web_safe_profile, apply_profile_to_selection as apply_web_safe_profile_to_selection,
    action_error as web_safe_action_error, collect_warning_items as collect_web_safe_warning_items,
    export_named_profile as export_named_web_safe_profile,
    import_profile as import_web_safe_profile, issues_for_binding as web_safe_issues_for_binding,
    prefers_internal_label as web_safe_prefers_internal_label,
    preview_profile_diff as preview_web_safe_profile_diff,
    preview_profile_diff_for_selection as preview_web_safe_profile_diff_for_selection,
    profile_export_status as web_safe_profile_export_status,
    profile_import_status as web_safe_profile_import_status,
    profile_preview_status as web_safe_profile_preview_status, render_warning_row, WebSafeProfile,
    WebSafeSummary, WebSafeWarningItem, WebSafeWarningRow,
    summarize as summarize_web_safe, uses_external_label as web_safe_uses_external_label,
};

#[derive(Clone)]
pub(crate) struct PreviewImageCacheEntry {
    texture: Option<egui::TextureHandle>,
    error: Option<String>,
}

impl std::fmt::Debug for PreviewImageCacheEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreviewImageCacheEntry")
            .field("texture_loaded", &self.texture.is_some())
            .field("error", &self.error)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct UiLinkEditorState {
    pub open: bool,
    pub window_template_open: bool,
    pub kp_template_open: bool,
    pub kp_binding_open: bool,
    pub kp_viewer_open: bool,
    pub windows: Vec<UiKpzWindowRow>,
    pub selected_window_id: Option<i64>,
    pub templates: Vec<UiScreenTemplateRow>,
    pub selected_template_id: Option<i64>,
    pub linked_templates: Vec<UiKpzTemplateLinkRow>,
    pub kp_templates: Vec<UiKpTemplateRow>,
    pub selected_kp_template_id: Option<i64>,
    pub selected_kp_binding_template_id: Option<i64>,
    pub kp_template_windows: Vec<UiKpTemplateWindowRow>,
    pub kp_binding_template_windows: Vec<UiKpTemplateWindowRow>,
    pub kpz_kp_template_link: Option<UiKpzKpTemplateLinkRow>,
    pub template_code: String,
    pub template_title: String,
    pub kp_template_code: String,
    pub kp_template_title: String,
    pub kp_template_description: String,
    pub window_code: String,
    pub window_title: String,
    pub window_description: String,
    pub groups_selected: BTreeSet<i32>,
    pub regs_available: Vec<RegRow>,
    pub regs_filter: String,
    pub regs_group_filter: Option<i32>,
    pub reg_pick_one: Option<i32>,
    pub regs_selected: BTreeSet<i32>,
    pub bindings: Vec<UiWindowBindingRow>,
    pub status: Option<String>,
    pub err: Option<String>,
    pub dirty: bool,
    pub help_open: bool,
    pub kp_binding_help_open: bool,
    pub selected_binding_reg_id: Option<i32>,
    pub selected_binding_reg_ids: BTreeSet<i32>,
    pub batch_w_input: String,
    pub batch_h_input: String,
    pub batch_gap_input: String,
    pub drag_binding_reg_id: Option<i32>,
    pub drag_offset: Option<egui::Vec2>,
    pub drag_resize_mode: bool,
    pub drag_group_mode: bool,
    pub drag_group_start: Option<egui::Pos2>,
    pub drag_group_positions: BTreeMap<i32, (i32, i32)>,
    pub drag_select_mode: bool,
    pub drag_select_start: Option<egui::Pos2>,
    pub drag_select_additive: bool,
    pub preview_edit_reg_id: Option<i32>,
    pub layout_open: bool,
    pub layout_focus_request: bool,
    pub preview_open: bool,
    pub live_values: BTreeMap<i32, Option<f64>>,
    pub trend_history: BTreeMap<i32, Vec<f64>>,
    pub cmd_inputs: BTreeMap<i32, String>,
    pub last_cmd_result: BTreeMap<i32, String>,
    pub alarm_rules_by_reg: BTreeMap<i32, Vec<AlarmRuleRow>>,
    pub(crate) preview_image_cache: BTreeMap<String, PreviewImageCacheEntry>,
    pub bits16_open: bool,
    pub bits16_reg_id: Option<i32>,
    pub bits16_value: u16,
    pub poll_trace: String,
    pub trace_open: bool,
    pub io_timeout_ms: u64,
    pub web_safe_preview: bool,
    pub web_safe_muted_reg_ids: BTreeSet<i32>,
    pub web_safe_show_muted: bool,
    pub web_safe_profile_preview: Option<String>,
    pub web_safe_profile_preview_add: Vec<String>,
    pub web_safe_profile_preview_remove: Vec<String>,
    pub web_safe_changed_reg_ids: Vec<i32>,
    pub web_safe_changed_nav_idx: usize,
    pub web_safe_filter_labels: bool,
    pub web_safe_filter_size: bool,
    pub web_safe_filter_trend: bool,
    pub web_safe_filter_write: bool,
    pub template_editor_mode: bool,
    pub kp_template_editor_mode: bool,
    pub kp_binding_editor_mode: bool,
}

impl Default for UiLinkEditorState {
    fn default() -> Self {
        Self {
            open: false,
            window_template_open: false,
            kp_template_open: false,
            kp_binding_open: false,
            kp_viewer_open: false,
            windows: Vec::new(),
            selected_window_id: None,
            templates: Vec::new(),
            selected_template_id: None,
            linked_templates: Vec::new(),
            kp_templates: Vec::new(),
            selected_kp_template_id: None,
            selected_kp_binding_template_id: None,
            kp_template_windows: Vec::new(),
            kp_binding_template_windows: Vec::new(),
            kpz_kp_template_link: None,
            template_code: String::new(),
            template_title: String::new(),
            kp_template_code: String::new(),
            kp_template_title: String::new(),
            kp_template_description: String::new(),
            window_code: String::new(),
            window_title: String::new(),
            window_description: String::new(),
            groups_selected: BTreeSet::new(),
            regs_available: Vec::new(),
            regs_filter: String::new(),
            regs_group_filter: None,
            reg_pick_one: None,
            regs_selected: BTreeSet::new(),
            bindings: Vec::new(),
            status: None,
            err: None,
            dirty: false,
            help_open: false,
            kp_binding_help_open: false,
            selected_binding_reg_id: None,
            selected_binding_reg_ids: BTreeSet::new(),
            batch_w_input: "120".to_string(),
            batch_h_input: "34".to_string(),
            batch_gap_input: "8".to_string(),
            drag_binding_reg_id: None,
            drag_offset: None,
            drag_resize_mode: false,
            drag_group_mode: false,
            drag_group_start: None,
            drag_group_positions: BTreeMap::new(),
            drag_select_mode: false,
            drag_select_start: None,
            drag_select_additive: false,
            preview_edit_reg_id: None,
            layout_open: true,
            layout_focus_request: false,
            preview_open: true,
            live_values: BTreeMap::new(),
            trend_history: BTreeMap::new(),
            cmd_inputs: BTreeMap::new(),
            last_cmd_result: BTreeMap::new(),
            alarm_rules_by_reg: BTreeMap::new(),
            preview_image_cache: BTreeMap::new(),
            bits16_open: false,
            bits16_reg_id: None,
            bits16_value: 0,
            poll_trace: String::new(),
            trace_open: true,
            io_timeout_ms: IO_REQ_TIMEOUT_MS,
            web_safe_preview: false,
            web_safe_muted_reg_ids: BTreeSet::new(),
            web_safe_show_muted: false,
            web_safe_profile_preview: None,
            web_safe_profile_preview_add: Vec::new(),
            web_safe_profile_preview_remove: Vec::new(),
            web_safe_changed_reg_ids: Vec::new(),
            web_safe_changed_nav_idx: 0,
            web_safe_filter_labels: true,
            web_safe_filter_size: true,
            web_safe_filter_trend: true,
            web_safe_filter_write: true,
            template_editor_mode: false,
            kp_template_editor_mode: false,
            kp_binding_editor_mode: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum UiLinkEditorAction {
    ReloadWindows,
    SelectWindow(Option<i64>),
    ReloadTemplates,
    ReloadTemplateLinks,
    SelectTemplate(Option<i64>),
    LinkTemplateToKpz,
    UnlinkTemplateFromKpz,
    CreateWindowFromTemplate,
    SyncTemplateImagesToWindows,
    UpdateTemplateImagesInWindows,
    ReloadKpTemplates,
    SelectKpTemplate(Option<i64>),
    SelectKpBindingTemplate(Option<i64>),
    SaveKpTemplate,
    DeleteKpTemplate,
    ReloadKpTemplateWindows,
    AddWindowTemplateToKpTemplate,
    RemoveWindowTemplateFromKpTemplate { window_template_id: i64 },
    ApplyKpTemplateToKpz,
    UnlinkKpTemplateAndDeleteWindows,
    ReloadKpzKpTemplateLink,
    UpsertWindow,
    DeleteWindow,
    ReloadRegs,
    SaveAll,
    PollNow,
    SendTu { reg_id: i32, on: bool },
    WriteValue { reg_id: i32, val: f64 },
}

impl UiLinkEditorState {
    fn clear_web_safe_profile_preview(&mut self) {
        self.web_safe_profile_preview = None;
        self.web_safe_profile_preview_add.clear();
        self.web_safe_profile_preview_remove.clear();
    }

    fn set_web_safe_profile_preview(
        &mut self,
        preview: String,
        will_mute: Vec<String>,
        will_unmute: Vec<String>,
    ) {
        self.web_safe_profile_preview = Some(preview);
        self.web_safe_profile_preview_add = will_mute;
        self.web_safe_profile_preview_remove = will_unmute;
    }

    fn clear_web_safe_changed_trail(&mut self) {
        self.web_safe_changed_reg_ids.clear();
        self.web_safe_changed_nav_idx = 0;
    }

    pub fn clear_for_new_window(&mut self) {
        self.selected_window_id = None;
        self.window_code.clear();
        self.window_title.clear();
        self.window_description.clear();
        self.template_code.clear();
        self.template_title.clear();
        self.kp_template_code.clear();
        self.kp_template_title.clear();
        self.kp_template_description.clear();
        self.selected_kp_template_id = None;
        self.selected_kp_binding_template_id = None;
        self.kp_template_windows.clear();
        self.kp_binding_template_windows.clear();
        self.groups_selected.clear();
        self.regs_available.clear();
        self.regs_group_filter = None;
        self.reg_pick_one = None;
        self.regs_selected.clear();
        self.bindings.clear();
        self.selected_binding_reg_id = None;
        self.selected_binding_reg_ids.clear();
        self.drag_binding_reg_id = None;
        self.drag_offset = None;
        self.drag_resize_mode = false;
        self.drag_group_mode = false;
        self.drag_group_start = None;
        self.drag_group_positions.clear();
        self.drag_select_mode = false;
        self.drag_select_start = None;
        self.drag_select_additive = false;
        self.preview_edit_reg_id = None;
        self.layout_open = true;
        self.layout_focus_request = false;
        self.web_safe_muted_reg_ids.clear();
        self.clear_web_safe_profile_preview();
        self.clear_web_safe_changed_trail();
        self.preview_open = true;
        self.live_values.clear();
        self.trend_history.clear();
        self.cmd_inputs.clear();
        self.last_cmd_result.clear();
        self.alarm_rules_by_reg.clear();
        self.bits16_open = false;
        self.bits16_reg_id = None;
        self.bits16_value = 0;
        self.poll_trace.clear();
        self.trace_open = true;
        self.err = None;
        self.status = None;
        self.dirty = false;
        self.kp_binding_help_open = false;
        self.kp_viewer_open = false;
    }

    pub(crate) fn sync_web_safe_muted_from_bindings(&mut self) {
        self.web_safe_muted_reg_ids = self
            .bindings
            .iter()
            .filter(|b| b.web_safe_muted)
            .map(|b| b.reg_id)
            .collect();
    }

    fn set_web_safe_muted(&mut self, reg_id: i32, muted: bool) {
        let mut changed = false;
        if muted {
            changed = self.web_safe_muted_reg_ids.insert(reg_id) || changed;
        } else {
            changed = self.web_safe_muted_reg_ids.remove(&reg_id) || changed;
        }
        if let Some(binding) = self.bindings.iter_mut().find(|b| b.reg_id == reg_id) {
            changed = binding.web_safe_muted != muted || changed;
            binding.web_safe_muted = muted;
        }
        if changed {
            self.dirty = true;
        }
    }

    fn selected_groups_caption(&self, groups: &[GroupRow]) -> String {
        if self.groups_selected.is_empty() {
            return "<none>".to_string();
        }
        let mut labels: Vec<String> = groups
            .iter()
            .filter(|g| self.groups_selected.contains(&g.id))
            .map(|g| {
                if g.name.is_empty() {
                    g.id.to_string()
                } else {
                    format!("{}:{}", g.id, g.name)
                }
            })
            .collect();
        if labels.is_empty() {
            return format!("{} selected", self.groups_selected.len());
        }
        labels.sort();
        if labels.len() <= 3 {
            return labels.join(", ");
        }
        format!("{}, ... (+{})", labels[0..3].join(", "), labels.len() - 3)
    }

    fn add_selected_regs_to_bindings(&mut self) {
        let mut max_pos = self.bindings.iter().map(|b| b.pos).max().unwrap_or(0);
        for r in &self.regs_available {
            if !self.regs_selected.contains(&r.id) {
                continue;
            }
            if self.bindings.iter().any(|b| b.reg_id == r.id) {
                continue;
            }
            max_pos += 10;
            let idx = self.bindings.len() as i32;
            self.bindings.push(UiWindowBindingRow {
                reg_id: r.id,
                is_text: false,
                pos: max_pos,
                x: 20 + (idx % 4) * 130,
                y: 20 + (idx / 4) * 52,
                w: 120,
                h: 34,
                visible: true,
                writable: false,
                label_override: None,
                unit: None,
                fmt: None,
                scale_max: None,
                component_kind: None,
                web_safe_muted: false,
                reg_name: r.name.clone(),
                reg_mb: r.mb,
                reg_n_mb: r.n_mb.unwrap_or(0),
                reg_tip: r.tip,
                reg_bits: r.bits,
            });
            self.dirty = true;
        }
        self.bindings.sort_by_key(|b| (b.pos, b.reg_id));
    }

    fn add_text_item(&mut self) {
        let mut max_pos = self.bindings.iter().map(|b| b.pos).max().unwrap_or(0);
        max_pos += 10;
        let min_reg = self.bindings.iter().map(|b| b.reg_id).min().unwrap_or(0);
        let reg_id = if min_reg <= -1 { min_reg - 1 } else { -1 };
        let idx = self.bindings.len() as i32;
        self.bindings.push(UiWindowBindingRow {
            reg_id,
            is_text: true,
            pos: max_pos,
            x: 20 + (idx % 4) * 130,
            y: 20 + (idx / 4) * 52,
            w: 120,
            h: 34,
            visible: true,
            writable: false,
            label_override: Some("Label".to_string()),
            unit: None,
            fmt: None,
            scale_max: None,
            component_kind: None,
            web_safe_muted: false,
            reg_name: "Text".to_string(),
            reg_mb: 0,
            reg_n_mb: 0,
            reg_tip: 0,
            reg_bits: None,
        });
        self.selected_binding_reg_id = Some(reg_id);
        self.selected_binding_reg_ids.insert(reg_id);
        self.dirty = true;
    }

    fn add_image_item(&mut self) {
        let mut max_pos = self.bindings.iter().map(|b| b.pos).max().unwrap_or(0);
        max_pos += 10;
        let min_reg = self.bindings.iter().map(|b| b.reg_id).min().unwrap_or(0);
        let reg_id = if min_reg <= -1 { min_reg - 1 } else { -1 };
        let (layout_w, layout_h) = self
            .bindings
            .iter()
            .filter(|b| !is_image_item(b))
            .fold((640, 360), |(mw, mh), b| {
                ((b.x + b.w + 32).max(mw), (b.y + b.h + 32).max(mh))
            });
        self.bindings.push(UiWindowBindingRow {
            reg_id,
            is_text: true,
            pos: max_pos,
            x: 0,
            y: 0,
            w: layout_w,
            h: layout_h,
            visible: true,
            writable: false,
            label_override: Some("ui_images/scheme.png".to_string()),
            unit: None,
            fmt: Some("contain".to_string()),
            scale_max: Some(1.0),
            component_kind: Some("image".to_string()),
            web_safe_muted: false,
            reg_name: "Image".to_string(),
            reg_mb: 0,
            reg_n_mb: 0,
            reg_tip: 0,
            reg_bits: None,
        });
        self.selected_binding_reg_id = Some(reg_id);
        self.selected_binding_reg_ids.insert(reg_id);
        self.dirty = true;
    }

    fn move_binding(&mut self, reg_id: i32, dir: i32) {
        let mut idx = None;
        for (i, b) in self.bindings.iter().enumerate() {
            if b.reg_id == reg_id {
                idx = Some(i);
                break;
            }
        }
        let Some(i) = idx else { return; };
        let j = if dir < 0 {
            if i == 0 {
                return;
            }
            i - 1
        } else {
            if i + 1 >= self.bindings.len() {
                return;
            }
            i + 1
        };
        self.bindings.swap(i, j);
        for (k, b) in self.bindings.iter_mut().enumerate() {
            b.pos = ((k as i32) + 1) * 10;
        }
        self.dirty = true;
    }

    fn select_all_bindings(&mut self) {
        self.selected_binding_reg_ids.clear();
        for b in &self.bindings {
            self.selected_binding_reg_ids.insert(b.reg_id);
        }
    }

    fn clear_binding_selection(&mut self) {
        self.selected_binding_reg_ids.clear();
    }

    fn batch_set_size(&mut self) {
        let w = match self.batch_w_input.trim().parse::<i32>() {
            Ok(v) => v.clamp(8, 2000),
            Err(_) => return,
        };
        let h = match self.batch_h_input.trim().parse::<i32>() {
            Ok(v) => v.clamp(8, 1200),
            Err(_) => return,
        };
        let mut changed = false;
        for b in &mut self.bindings {
            if self.selected_binding_reg_ids.contains(&b.reg_id) {
                if b.w != w || b.h != h {
                    b.w = w;
                    b.h = h;
                    changed = true;
                }
            }
        }
        if changed {
            self.dirty = true;
        }
    }

    fn batch_arrange(&mut self, vertical: bool) {
        let gap = self
            .batch_gap_input
            .trim()
            .parse::<i32>()
            .unwrap_or(8)
            .clamp(0, 500);
        let mut ids: Vec<i32> = self
            .bindings
            .iter()
            .filter(|b| self.selected_binding_reg_ids.contains(&b.reg_id))
            .map(|b| b.reg_id)
            .collect();
        if ids.len() < 2 {
            return;
        }
        let mut anchor_x = i32::MAX;
        let mut anchor_y = i32::MAX;
        for b in &self.bindings {
            if self.selected_binding_reg_ids.contains(&b.reg_id) {
                anchor_x = anchor_x.min(b.x);
                anchor_y = anchor_y.min(b.y);
            }
        }
        ids.sort_by_key(|id| {
            self.bindings
                .iter()
                .find(|b| b.reg_id == *id)
                .map(|b| if vertical { (b.y, b.x) } else { (b.x, b.y) })
                .unwrap_or((0, 0))
        });
        let mut cur_x = anchor_x;
        let mut cur_y = anchor_y;
        let mut changed = false;
        for id in ids {
            if let Some(b) = self.bindings.iter_mut().find(|b| b.reg_id == id) {
                if b.x != cur_x || b.y != cur_y {
                    b.x = cur_x;
                    b.y = cur_y;
                    changed = true;
                }
                if vertical {
                    cur_y += b.h.max(8) + gap;
                } else {
                    cur_x += b.w.max(8) + gap;
                }
            }
        }
        if changed {
            self.dirty = true;
        }
    }

    fn fit_selected_images_to_layout(&mut self) {
        let (layout_w, layout_h) = self
            .bindings
            .iter()
            .filter(|b| !is_image_item(b))
            .fold((640, 360), |(mw, mh), b| {
                ((b.x + b.w + 32).max(mw), (b.y + b.h + 32).max(mh))
            });
        let mut changed = false;
        for b in &mut self.bindings {
            if self.selected_binding_reg_ids.contains(&b.reg_id) && is_image_item(b) {
                if b.x != 0 || b.y != 0 || b.w != layout_w || b.h != layout_h {
                    b.x = 0;
                    b.y = 0;
                    b.w = layout_w;
                    b.h = layout_h;
                    changed = true;
                }
            }
        }
        if changed {
            self.status = Some(format!("selected image background fitted to {}x{}", layout_w, layout_h));
            self.dirty = true;
        }
    }

    fn send_selected_images_to_back(&mut self) {
        let selected = self.selected_binding_reg_ids.clone();
        let mut changed = false;
        self.bindings.sort_by_key(|b| {
            let selected_image = selected.contains(&b.reg_id) && is_image_item(b);
            (if selected_image { 0 } else { 1 }, b.pos, b.reg_id)
        });
        for (k, b) in self.bindings.iter_mut().enumerate() {
            let new_pos = ((k as i32) + 1) * 10;
            if b.pos != new_pos {
                b.pos = new_pos;
                changed = true;
            }
        }
        if changed {
            self.status = Some("selected images sent to background layer".to_string());
            self.dirty = true;
        }
    }

    fn remove_binding(&mut self, reg_id: i32) {
        self.bindings.retain(|b| b.reg_id != reg_id);
        if self.selected_binding_reg_id == Some(reg_id) {
            self.selected_binding_reg_id = None;
        }
        self.selected_binding_reg_ids.remove(&reg_id);
        self.web_safe_muted_reg_ids.remove(&reg_id);
        if self.drag_binding_reg_id == Some(reg_id) {
            self.drag_binding_reg_id = None;
            self.drag_offset = None;
            self.drag_resize_mode = false;
        }
        self.drag_group_positions.remove(&reg_id);
        if self.drag_group_positions.is_empty() {
            self.drag_group_mode = false;
            self.drag_group_start = None;
        }
        for (k, b) in self.bindings.iter_mut().enumerate() {
            b.pos = ((k as i32) + 1) * 10;
        }
        self.dirty = true;
    }
}

fn show_layout_window(ctx: &egui::Context, state: &mut UiLinkEditorState) {
    if !state.window_template_open || !state.layout_open {
        return;
    }

    let mut layout_open = state.layout_open;
    let mut w = egui::Window::new("UI Layout")
        .open(&mut layout_open)
        .order(egui::Order::Foreground)
        .resizable(true)
        .default_size([450.0, 350.0])
        .default_pos([80.0, 80.0]);
    if state.layout_focus_request {
        w = w.current_pos([80.0, 80.0]);
    }
    w.show(ctx, |ui| {
            ui.label("Ctrl+LMB marks elements. Drag selected element to move group. Drag empty area to select by rectangle.");
            ui.horizontal(|ui| {
                let can_add_layout_items = !state.kp_template_editor_mode;
                let add_text_btn = ui
                    .add_enabled(can_add_layout_items, egui::Button::new("Add text"))
                    .on_hover_text(if can_add_layout_items {
                        "Add text item to the current window/template layout"
                    } else {
                        "Text/image items are saved only in Window template editor, not KP template editor"
                    });
                if add_text_btn.clicked() {
                    state.add_text_item();
                }
                let add_image_btn = ui
                    .add_enabled(can_add_layout_items, egui::Button::new("Add image"))
                    .on_hover_text(if can_add_layout_items {
                        "Add image item to the current window/template layout"
                    } else {
                        "Text/image items are saved only in Window template editor, not KP template editor"
                    });
                if add_image_btn.clicked() {
                    state.add_image_item();
                }
                ui.separator();
                let sel_id = state
                    .selected_binding_reg_id
                    .or_else(|| state.selected_binding_reg_ids.iter().next().copied());
                if let Some(reg_id) = sel_id {
                    ui.label(format!("Label reg {}:", reg_id));
                    if let Some(b) = state.bindings.iter_mut().find(|x| x.reg_id == reg_id) {
                        let mut label = b.label_override.clone().unwrap_or_default();
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut label)
                                .desired_width(260.0)
                                .hint_text("override label"),
                        );
                        if resp.changed() {
                            let t = label.trim().to_string();
                            b.label_override = if t.is_empty() { None } else { Some(t) };
                            state.dirty = true;
                        }
                        if ui.small_button("Clear").clicked() {
                            b.label_override = None;
                            state.dirty = true;
                        }
                    }
                } else {
                    ui.label("Label: select one element");
                }
            });
            let avail = ui.available_size_before_wrap();
            let canvas_size = egui::vec2(avail.x.max(320.0), avail.y.max(240.0));
            let (rect, response) = ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 24, 34));
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(80)),
            );

            let mut hit_move: Option<(i32, egui::Vec2)> = None;
            let mut hit_resize: Option<(i32, egui::Vec2)> = None;
            if let Some(pos) = response.interact_pointer_pos() {
                let mut hit_bindings = state.bindings.clone();
                // Prefer controls over background images when hit-testing overlapping items.
                hit_bindings.sort_by_key(|b| (if is_image_item(b) { 0 } else { 1 }, b.pos, b.reg_id));
                for b in hit_bindings.iter().rev() {
                    let is_bool = b.reg_tip == 0 && b.reg_bits.is_some();
                    let min_w = if is_bool { 8 } else { 30 };
                    let min_h = if is_bool { 8 } else { 18 };
                    let w = b.w.max(min_w) as f32;
                    let h = b.h.max(min_h) as f32;
                    let top_left = rect.min + egui::vec2(b.x as f32, b.y as f32);
                    let r = egui::Rect::from_min_size(top_left, egui::vec2(w, h));
                    let handle = egui::Rect::from_min_size(
                        r.max - egui::vec2(14.0, 14.0),
                        egui::vec2(14.0, 14.0),
                    );
                    if handle.contains(pos) {
                        hit_resize = Some((b.reg_id, r.max - pos));
                        break;
                    }
                    if r.contains(pos) {
                        hit_move = Some((b.reg_id, pos - top_left));
                        break;
                    }
                }
            }

            if response.clicked() {
                if let Some((reg_id, _)) = hit_resize.or(hit_move) {
                    state.selected_binding_reg_id = Some(reg_id);
                    let ctrl = ui.input(|i| i.modifiers.ctrl);
                    if ctrl {
                        if state.selected_binding_reg_ids.contains(&reg_id) {
                            state.selected_binding_reg_ids.remove(&reg_id);
                        } else {
                            state.selected_binding_reg_ids.insert(reg_id);
                        }
                    } else {
                        state.selected_binding_reg_ids.clear();
                        state.selected_binding_reg_ids.insert(reg_id);
                    }
                }
            }

            if response.drag_started() {
                if let Some((reg_id, off)) = hit_resize {
                    state.selected_binding_reg_id = Some(reg_id);
                    state.drag_binding_reg_id = Some(reg_id);
                    state.drag_resize_mode = true;
                    state.drag_group_mode = false;
                    state.drag_group_start = None;
                    state.drag_group_positions.clear();
                    state.drag_select_mode = false;
                    state.drag_select_start = None;
                    state.drag_offset = Some(off);
                } else if let Some((reg_id, off)) = hit_move {
                    state.selected_binding_reg_id = Some(reg_id);
                    if !state.selected_binding_reg_ids.contains(&reg_id) {
                        state.selected_binding_reg_ids.clear();
                        state.selected_binding_reg_ids.insert(reg_id);
                    }
                    let group_drag = state.selected_binding_reg_ids.len() > 1
                        && state.selected_binding_reg_ids.contains(&reg_id);
                    if group_drag {
                        state.drag_binding_reg_id = None;
                        state.drag_resize_mode = false;
                        state.drag_offset = Some(off);
                        state.drag_group_mode = true;
                        state.drag_group_start = response.interact_pointer_pos();
                        state.drag_group_positions.clear();
                        state.drag_select_mode = false;
                        state.drag_select_start = None;
                        for b in &state.bindings {
                            if state.selected_binding_reg_ids.contains(&b.reg_id) {
                                state.drag_group_positions.insert(b.reg_id, (b.x, b.y));
                            }
                        }
                    } else {
                        state.drag_binding_reg_id = Some(reg_id);
                        state.drag_resize_mode = false;
                        state.drag_group_mode = false;
                        state.drag_group_start = None;
                        state.drag_group_positions.clear();
                        state.drag_select_mode = false;
                        state.drag_select_start = None;
                        state.drag_offset = Some(off);
                    }
                } else if let Some(start) = response.interact_pointer_pos() {
                    state.drag_binding_reg_id = None;
                    state.drag_offset = None;
                    state.drag_resize_mode = false;
                    state.drag_group_mode = false;
                    state.drag_group_start = None;
                    state.drag_group_positions.clear();
                    state.drag_select_mode = true;
                    state.drag_select_start = Some(start);
                    state.drag_select_additive = ui.input(|i| i.modifiers.ctrl);
                }
            }

            if !ui.input(|i| i.pointer.primary_down()) {
                if state.drag_select_mode {
                    let end = ui
                        .input(|i| i.pointer.latest_pos())
                        .or(state.drag_select_start);
                    if let (Some(start), Some(end)) = (state.drag_select_start, end) {
                        let sel = egui::Rect::from_two_pos(start, end);
                        if !state.drag_select_additive {
                            state.selected_binding_reg_ids.clear();
                        }
                        for b in &state.bindings {
                            let is_bool = b.reg_tip == 0 && b.reg_bits.is_some();
                            let min_w = if is_bool { 8 } else { 30 };
                            let min_h = if is_bool { 8 } else { 18 };
                            let bw = b.w.max(min_w) as f32;
                            let bh = b.h.max(min_h) as f32;
                            let top_left = rect.min + egui::vec2(b.x as f32, b.y as f32);
                            let r = egui::Rect::from_min_size(top_left, egui::vec2(bw, bh));
                            if r.intersects(sel) {
                                state.selected_binding_reg_ids.insert(b.reg_id);
                            }
                        }
                        if let Some(id) = state.selected_binding_reg_ids.iter().next().copied() {
                            state.selected_binding_reg_id = Some(id);
                        }
                    }
                }
                state.drag_binding_reg_id = None;
                state.drag_offset = None;
                state.drag_resize_mode = false;
                state.drag_group_mode = false;
                state.drag_group_start = None;
                state.drag_group_positions.clear();
                state.drag_select_mode = false;
                state.drag_select_start = None;
                state.drag_select_additive = false;
            }

            if state.drag_group_mode {
                if let (Some(start), Some(pos)) = (state.drag_group_start, response.interact_pointer_pos()) {
                    let dx = (pos.x - start.x).round() as i32;
                    let dy = (pos.y - start.y).round() as i32;
                    let mut changed = false;
                    for b in &mut state.bindings {
                        let Some((x0, y0)) = state.drag_group_positions.get(&b.reg_id).copied() else {
                            continue;
                        };
                        let is_bool = b.reg_tip == 0 && b.reg_bits.is_some();
                        let min_w = if is_bool { 8.0 } else { 30.0 };
                        let min_h = if is_bool { 8.0 } else { 18.0 };
                        let bw = b.w.max(min_w as i32) as f32;
                        let bh = b.h.max(min_h as i32) as f32;
                        let max_x = (rect.width() - bw).max(0.0).round() as i32;
                        let max_y = (rect.height() - bh).max(0.0).round() as i32;
                        let nx = (x0 + dx).clamp(0, max_x);
                        let ny = (y0 + dy).clamp(0, max_y);
                        if b.x != nx || b.y != ny {
                            b.x = nx;
                            b.y = ny;
                            changed = true;
                        }
                    }
                    if changed {
                        state.dirty = true;
                    }
                }
            } else if let (Some(reg_id), Some(pos)) = (state.drag_binding_reg_id, response.interact_pointer_pos()) {
                if let Some(b) = state.bindings.iter_mut().find(|x| x.reg_id == reg_id) {
                    let off = state.drag_offset.unwrap_or(egui::vec2(10.0, 10.0));
                    let is_bool = b.reg_tip == 0 && b.reg_bits.is_some();
                    let min_w = if is_bool { 8.0 } else { 30.0 };
                    let min_h = if is_bool { 8.0 } else { 18.0 };
                    if state.drag_resize_mode {
                        // Do not clamp resize to current canvas bounds:
                        // users may need larger controls and then reposition them.
                        let max_w = 2000.0;
                        let max_h = 1200.0;
                        let nw = (pos.x - rect.min.x - b.x as f32 + off.x).clamp(min_w, max_w).round() as i32;
                        let nh = (pos.y - rect.min.y - b.y as f32 + off.y).clamp(min_h, max_h).round() as i32;
                        if b.w != nw || b.h != nh {
                            b.w = nw;
                            b.h = nh;
                            state.dirty = true;
                        }
                    } else {
                        let bw = b.w.max(min_w as i32) as f32;
                        let bh = b.h.max(min_h as i32) as f32;
                        let max_x = (rect.width() - bw).max(0.0);
                        let max_y = (rect.height() - bh).max(0.0);
                        let nx = (pos.x - rect.min.x - off.x).clamp(0.0, max_x).round() as i32;
                        let ny = (pos.y - rect.min.y - off.y).clamp(0.0, max_y).round() as i32;
                        if b.x != nx || b.y != ny {
                            b.x = nx;
                            b.y = ny;
                            state.dirty = true;
                        }
                    }
                }
            }

            if state.drag_select_mode {
                if let (Some(start), Some(end)) =
                    (state.drag_select_start, ui.input(|i| i.pointer.latest_pos()))
                {
                    let sel = egui::Rect::from_two_pos(start, end);
                    painter.rect_stroke(
                        sel,
                        0.0,
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(120, 190, 255)),
                    );
                }
            }

            let mut layout_bindings = state.bindings.clone();
            // Draw image items first so UI Layout matches Preview/background behavior.
            layout_bindings.sort_by_key(|b| (if is_image_item(b) { 0 } else { 1 }, b.pos, b.reg_id));
            for b in &layout_bindings {
                let is_bool = b.reg_tip == 0 && b.reg_bits.is_some();
                let min_w = if is_bool { 8 } else { 30 };
                let min_h = if is_bool { 8 } else { 18 };
                let bw = b.w.max(min_w) as f32;
                let bh = b.h.max(min_h) as f32;
                let top_left = rect.min + egui::vec2(b.x as f32, b.y as f32);
                let r = egui::Rect::from_min_size(top_left, egui::vec2(bw, bh));
                let selected = state.selected_binding_reg_ids.contains(&b.reg_id)
                    || state.selected_binding_reg_id == Some(b.reg_id);
                let is_image = is_image_item(b);
                if is_image {
                    let lead = b
                        .label_override
                        .as_ref()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .unwrap_or(&b.reg_name);
                    let alpha = (b.scale_max.unwrap_or(1.0).clamp(0.0, 1.0) * 255.0) as u8;
                    let cache_version = image_cache_version(b);
                    let entry = load_preview_image_texture(ui.ctx(), state, lead, &cache_version);
                    if let Some(texture) = entry.texture {
                        painter.rect_filled(r, 3.0, egui::Color32::from_rgb(8, 12, 18));
                        let fit_rect = image_fit_rect(r, texture.size_vec2(), b.fmt.as_deref().unwrap_or("contain"));
                        painter.with_clip_rect(r).image(
                            texture.id(),
                            fit_rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                        );
                    } else {
                        painter.rect_filled(r, 3.0, egui::Color32::from_rgb(19, 28, 42));
                        let diag = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(110, 170, 220, 90));
                        painter.line_segment([r.left_top(), r.right_bottom()], diag);
                        painter.line_segment([r.right_top(), r.left_bottom()], diag);
                        painter.text(
                            r.center(),
                            egui::Align2::CENTER_CENTER,
                            "IMAGE",
                            egui::TextStyle::Body.resolve(ui.style()),
                            egui::Color32::from_rgb(220, 238, 255),
                        );
                    }
                    painter.rect_stroke(
                        r,
                        3.0,
                        egui::Stroke::new(
                            if selected { 2.0 } else { 1.0 },
                            if selected {
                                egui::Color32::from_rgb(120, 196, 255)
                            } else {
                                egui::Color32::from_rgb(80, 130, 180)
                            },
                        ),
                    );
                } else {
                    let fill = if selected {
                        egui::Color32::from_rgb(90, 146, 255)
                    } else {
                        egui::Color32::from_rgb(70, 78, 98)
                    };
                    painter.rect_filled(r, 3.0, fill);
                    painter.rect_stroke(
                        r,
                        3.0,
                        egui::Stroke::new(1.0, egui::Color32::WHITE),
                    );
                }
                let handle = egui::Rect::from_min_size(
                    r.max - egui::vec2(14.0, 14.0),
                    egui::vec2(14.0, 14.0),
                );
                painter.rect_filled(handle, 1.0, egui::Color32::WHITE);
                let lead = b
                    .label_override
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&b.reg_name);
                if !is_image {
                    painter.text(
                        r.left_center() + egui::vec2(-6.0, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        lead,
                        egui::TextStyle::Body.resolve(ui.style()),
                        egui::Color32::LIGHT_GRAY,
                    );
                }
                let label = b
                    .label_override
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&b.reg_name)
                    .to_string();
                if !is_image {
                    painter.text(
                        r.center(),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::TextStyle::Body.resolve(ui.style()),
                        egui::Color32::WHITE,
                    );
                }
            }
        });
    state.layout_open = layout_open;
    if state.layout_focus_request {
        state.layout_focus_request = false;
    }
}

fn fmt_binding_number(binding: &UiWindowBindingRow, value: f64, is_u16_holding: bool) -> String {
    if is_u16_holding {
        return format!("{}", value.round().clamp(0.0, 65535.0) as i64);
    }
    match binding.fmt.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some("0") => format!("{}", value.round() as i64),
        Some("0.0") => format!("{value:.1}"),
        Some("0.00") => format!("{value:.2}"),
        Some("0.000") => format!("{value:.3}"),
        _ => format!("{value:.3}"),
    }
}

fn fmt_binding_live(binding: &UiWindowBindingRow, live: Option<f64>, is_u16_holding: bool) -> String {
    let base = match live {
        Some(value) => fmt_binding_number(binding, value, is_u16_holding),
        None => "-".to_string(),
    };
    let unit = binding
        .unit
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match unit {
        Some(unit) if base != "-" => format!("{base} {unit}"),
        _ => base,
    }
}

pub fn push_trend_sample(state: &mut UiLinkEditorState, reg_id: i32, value: f64) {
    const TREND_MAX_POINTS: usize = 48;
    let history = state.trend_history.entry(reg_id).or_default();
    history.push(value);
    if history.len() > TREND_MAX_POINTS {
        let drop_n = history.len() - TREND_MAX_POINTS;
        history.drain(0..drop_n);
    }
}

fn preview_component_kind(binding: &UiWindowBindingRow) -> &str {
    binding.component_kind.as_deref().unwrap_or("auto")
}

fn is_image_item(binding: &UiWindowBindingRow) -> bool {
    (binding.is_text || binding.reg_id <= 0) && preview_component_kind(binding) == "image"
}

fn preview_image_candidates(path: &str) -> Vec<PathBuf> {
    let p = PathBuf::from(path.trim());
    if p.is_absolute() {
        return vec![p];
    }

    let mut out = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join(&p));
    }
    if let Ok(exe) = std::env::current_exe() {
        for base in exe.ancestors().filter(|x| x.is_dir()) {
            out.push(base.join(&p));
        }
    }
    out.push(Path::new(env!("CARGO_MANIFEST_DIR")).join(&p));
    out
}

fn resolve_preview_image_path(path: &str) -> PathBuf {
    let candidates = preview_image_candidates(path);
    candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .unwrap_or_else(|| candidates.into_iter().next().unwrap_or_else(|| PathBuf::from(path.trim())))
}

fn image_cache_version(binding: &UiWindowBindingRow) -> String {
    format!(
        "{}:{}:{}:{}:{}:{:.3}",
        binding.x,
        binding.y,
        binding.w,
        binding.h,
        binding.fmt.as_deref().unwrap_or("contain"),
        binding.scale_max.unwrap_or(1.0).clamp(0.0, 1.0)
    )
}

fn load_preview_image_texture(
    ctx: &egui::Context,
    state: &mut UiLinkEditorState,
    image_path: &str,
    cache_version: &str,
) -> PreviewImageCacheEntry {
    let path = image_path.trim();
    if path.is_empty() {
        return PreviewImageCacheEntry {
            texture: None,
            error: Some("empty image path".to_string()),
        };
    }
    let full_path = resolve_preview_image_path(path);
    let meta_version = fs::metadata(&full_path)
        .ok()
        .map(|m| {
            let modified_ms = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis())
                .unwrap_or(0);
            format!("{}:{modified_ms}", m.len())
        })
        .unwrap_or_else(|| "missing".to_string());
    let key = format!("{}|{}|{}", full_path.to_string_lossy(), meta_version, cache_version);
    if let Some(entry) = state.preview_image_cache.get(&key) {
        return entry.clone();
    }
    if state.preview_image_cache.len() > 64 {
        state.preview_image_cache.clear();
    }

    let entry = match ImageReader::open(&full_path)
        .map_err(|e| e.to_string())
        .and_then(|reader| reader.decode().map_err(|e| e.to_string()))
    {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            let texture = ctx.load_texture(
                format!("preview-image:{key}"),
                color_image,
                egui::TextureOptions::LINEAR,
            );
            PreviewImageCacheEntry {
                texture: Some(texture),
                error: None,
            }
        }
        Err(e) => {
            let tried = preview_image_candidates(path)
                .into_iter()
                .take(4)
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(" | ");
            PreviewImageCacheEntry {
                texture: None,
                error: Some(format!("image load failed: {e}; tried: {tried}")),
            }
        }
    };
    state.preview_image_cache.insert(key, entry.clone());
    entry
}

fn image_fit_rect(rect: egui::Rect, texture_size: egui::Vec2, fit_mode: &str) -> egui::Rect {
    if fit_mode == "stretch" || texture_size.x <= 0.0 || texture_size.y <= 0.0 {
        return rect;
    }
    let sx = rect.width() / texture_size.x;
    let sy = rect.height() / texture_size.y;
    let scale = if fit_mode == "cover" { sx.max(sy) } else { sx.min(sy) };
    let size = texture_size * scale;
    egui::Rect::from_center_size(rect.center(), size)
}

fn preview_scale_max(binding: &UiWindowBindingRow) -> f64 {
    binding
        .scale_max
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(100.0)
}

fn binding_display_label(binding: &UiWindowBindingRow) -> &str {
    binding
        .label_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&binding.reg_name)
}

fn focus_binding_from_warning(state: &mut UiLinkEditorState, reg_id: i32) {
    state.selected_binding_reg_id = Some(reg_id);
    state.selected_binding_reg_ids.clear();
    state.selected_binding_reg_ids.insert(reg_id);
    state.layout_open = true;
    state.layout_focus_request = true;
}

fn set_changed_binding_navigation(state: &mut UiLinkEditorState, changed_reg_ids: Vec<i32>) {
    state.web_safe_changed_reg_ids = changed_reg_ids;
    state.web_safe_changed_nav_idx = 0;
    if let Some(reg_id) = state.web_safe_changed_reg_ids.first().copied() {
        focus_binding_from_warning(state, reg_id);
    }
}

fn navigate_changed_binding(state: &mut UiLinkEditorState, step: isize) {
    if state.web_safe_changed_reg_ids.is_empty() {
        return;
    }
    let len = state.web_safe_changed_reg_ids.len() as isize;
    let next = (state.web_safe_changed_nav_idx as isize + step).rem_euclid(len) as usize;
    state.web_safe_changed_nav_idx = next;
    if let Some(reg_id) = state.web_safe_changed_reg_ids.get(next).copied() {
        focus_binding_from_warning(state, reg_id);
    }
}

fn clear_changed_binding_navigation(state: &mut UiLinkEditorState) {
    state.clear_web_safe_changed_trail();
}

fn binding_has_active_web_warning(state: &UiLinkEditorState, binding: &UiWindowBindingRow) -> bool {
    !state.web_safe_muted_reg_ids.contains(&binding.reg_id) && !web_safe_issues_for_binding(binding).is_empty()
}

fn binding_has_muted_web_warning(state: &UiLinkEditorState, binding: &UiWindowBindingRow) -> bool {
    state.web_safe_muted_reg_ids.contains(&binding.reg_id) && !web_safe_issues_for_binding(binding).is_empty()
}

fn warning_item_matches_filters(state: &UiLinkEditorState, item: &WebSafeWarningItem) -> bool {
    let any_filter_on = state.web_safe_filter_labels
        || state.web_safe_filter_size
        || state.web_safe_filter_trend
        || state.web_safe_filter_write;
    if !any_filter_on {
        return true;
    }
    (state.web_safe_filter_labels && item.labels)
        || (state.web_safe_filter_size && item.size)
        || (state.web_safe_filter_trend && item.trend)
        || (state.web_safe_filter_write && item.write)
}

fn warning_navigation_target(
    state: &UiLinkEditorState,
    items: &[WebSafeWarningItem],
    filtered_indices: &[usize],
    step: isize,
) -> Option<i32> {
    if filtered_indices.is_empty() {
        return None;
    }
    let current = state.selected_binding_reg_id.and_then(|reg_id| {
        filtered_indices
            .iter()
            .position(|idx| items[*idx].reg_id == reg_id)
    });
    let len = filtered_indices.len() as isize;
    let next_pos = match current {
        Some(pos) => (pos as isize + step).rem_euclid(len) as usize,
        None if step >= 0 => 0,
        None => filtered_indices.len() - 1,
    };
    Some(items[filtered_indices[next_pos]].reg_id)
}

fn build_web_safe_report(
    state: &UiLinkEditorState,
    summary: Option<&WebSafeSummary>,
    active_items: &[WebSafeWarningItem],
    muted_items: &[WebSafeWarningItem],
) -> String {
    let scope = if let Some(window_id) = state.selected_window_id {
        format!(
            "Window {} [{}] {}",
            window_id,
            state.window_code.trim(),
            state.window_title.trim()
        )
    } else if let Some(template_id) = state.selected_template_id {
        format!(
            "Template {} [{}] {}",
            template_id,
            state.template_code.trim(),
            state.template_title.trim()
        )
    } else {
        "Window/template not selected".to_string()
    };

    let mut lines = vec![
        "SS7 web-safe report".to_string(),
        scope,
        format!("Web-safe enabled: {}", if state.web_safe_preview { "yes" } else { "no" }),
        format!("Unsaved changes: {}", if state.dirty { "yes" } else { "no" }),
    ];
    if let Some(summary) = summary {
        lines.push(summary.detail.clone());
        if !summary.breakdown.is_empty() {
            lines.push(format!("Breakdown: {}", summary.breakdown));
        }
        if !summary.fixes.is_empty() {
            lines.push(format!("Suggested fixes: {}", summary.fixes));
        }
    }
    lines.push(format!("Active warnings: {}", active_items.len()));
    lines.push(format!("Muted warnings: {}", muted_items.len()));

    if !active_items.is_empty() {
        lines.push(String::new());
        lines.push("Active warnings:".to_string());
        for item in active_items {
            lines.push(format!("- {}", item.message));
        }
    }
    if !muted_items.is_empty() {
        lines.push(String::new());
        lines.push("Muted warnings:".to_string());
        for item in muted_items {
            lines.push(format!("- {}", item.message));
        }
    }

    lines.join("\n")
}

fn web_safe_scope_label(state: &UiLinkEditorState) -> String {
    if let Some(window_id) = state.selected_window_id {
        format!("window_{window_id}")
    } else if let Some(template_id) = state.selected_template_id {
        format!("template_{template_id}")
    } else {
        "web_safe".to_string()
    }
}

fn build_web_safe_profile(state: &UiLinkEditorState) -> WebSafeProfile {
    WebSafeProfile {
        version: 1,
        scope: web_safe_scope_label(state),
        muted_reg_ids: state.web_safe_muted_reg_ids.iter().copied().collect(),
        show_muted: state.web_safe_show_muted,
        filter_labels: state.web_safe_filter_labels,
        filter_size: state.web_safe_filter_size,
        filter_trend: state.web_safe_filter_trend,
        filter_write: state.web_safe_filter_write,
    }
}

fn build_web_safe_profile_from_selection(state: &UiLinkEditorState) -> WebSafeProfile {
    let selected_ids = &state.selected_binding_reg_ids;
    WebSafeProfile {
        version: 1,
        scope: format!("{}_selection", web_safe_scope_label(state)),
        muted_reg_ids: state
            .web_safe_muted_reg_ids
            .iter()
            .copied()
            .filter(|reg_id| selected_ids.contains(reg_id))
            .collect(),
        show_muted: state.web_safe_show_muted,
        filter_labels: state.web_safe_filter_labels,
        filter_size: state.web_safe_filter_size,
        filter_trend: state.web_safe_filter_trend,
        filter_write: state.web_safe_filter_write,
    }
}

fn export_web_safe_profile(state: &UiLinkEditorState) -> Result<String, String> {
    let profile = build_web_safe_profile(state);
    export_named_web_safe_profile(&profile)
}

fn export_web_safe_profile_from_selection(state: &UiLinkEditorState) -> Result<String, String> {
    let profile = build_web_safe_profile_from_selection(state);
    export_named_web_safe_profile(&profile)
}

fn export_web_safe_report(
    state: &UiLinkEditorState,
    summary: Option<&WebSafeSummary>,
    active_items: &[WebSafeWarningItem],
    muted_items: &[WebSafeWarningItem],
) -> Result<String, String> {
    let report = build_web_safe_report(state, summary, active_items, muted_items);
    let report_dir = std::env::current_dir()
        .map_err(|e| format!("current dir failed: {e}"))?
        .join("web_safe_reports");
    fs::create_dir_all(&report_dir).map_err(|e| format!("create report dir failed: {e}"))?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("clock failed: {e}"))?
        .as_secs();
    let scope = state
        .selected_window_id
        .map(|id| format!("window_{id}"))
        .or_else(|| state.selected_template_id.map(|id| format!("template_{id}")))
        .unwrap_or_else(|| "web_safe".to_string());
    let path = report_dir.join(format!("{scope}_{ts}.txt"));
    fs::write(&path, report).map_err(|e| format!("write report failed: {e}"))?;
    Ok(path.display().to_string())
}

fn draw_web_safe_warning_badge(painter: &egui::Painter, rect: egui::Rect) {
    let badge = egui::Rect::from_min_size(
        egui::pos2(rect.max.x - 14.0, rect.min.y + 4.0),
        egui::vec2(10.0, 10.0),
    );
    painter.circle_filled(
        badge.center(),
        5.0,
        egui::Color32::from_rgb(255, 194, 73),
    );
    painter.text(
        badge.center(),
        egui::Align2::CENTER_CENTER,
        "!",
        egui::FontId::proportional(9.5),
        egui::Color32::from_rgb(46, 34, 8),
    );
}

fn draw_web_safe_muted_badge(painter: &egui::Painter, rect: egui::Rect) {
    let badge = egui::Rect::from_min_size(
        egui::pos2(rect.max.x - 38.0, rect.min.y + 4.0),
        egui::vec2(30.0, 12.0),
    );
    painter.rect_filled(
        badge,
        6.0,
        egui::Color32::from_rgba_unmultiplied(70, 82, 98, 220),
    );
    painter.rect_stroke(
        badge,
        6.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(124, 139, 158)),
    );
    painter.text(
        badge.center(),
        egui::Align2::CENTER_CENTER,
        "muted",
        egui::FontId::proportional(8.5),
        egui::Color32::from_rgb(226, 233, 241),
    );
}

fn show_preview_contents(ui: &mut egui::Ui, state: &mut UiLinkEditorState) -> Vec<UiLinkEditorAction> {
    let mut actions = Vec::new();
    let web_safe = state.web_safe_preview;
    let web_warning_items = if web_safe {
        collect_web_safe_warning_items(&state.bindings, &state.web_safe_muted_reg_ids, false)
    } else {
        Vec::new()
    };
    let muted_web_warning_items = if web_safe {
        collect_web_safe_warning_items(&state.bindings, &state.web_safe_muted_reg_ids, true)
    } else {
        Vec::new()
    };
    let web_summary = if web_safe {
        Some(summarize_web_safe(&state.bindings, &state.web_safe_muted_reg_ids))
    } else {
        None
    };
    let mut focus_warning_reg_id: Option<i32> = None;
    let selected_warning_reg_id = state.selected_binding_reg_id.filter(|reg_id| {
        state
            .bindings
            .iter()
            .find(|binding| binding.reg_id == *reg_id)
            .map(|binding| !web_safe_issues_for_binding(binding).is_empty())
            .unwrap_or(false)
    });
    let selected_warning_is_muted = selected_warning_reg_id
        .map(|reg_id| state.web_safe_muted_reg_ids.contains(&reg_id))
        .unwrap_or(false);
    let muted_warning_count = state
        .bindings
        .iter()
        .filter(|binding| {
            state.web_safe_muted_reg_ids.contains(&binding.reg_id)
                && !web_safe_issues_for_binding(binding).is_empty()
        })
        .count();
    let filtered_web_warning_indices: Vec<usize> = web_warning_items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| warning_item_matches_filters(state, item).then_some(idx))
        .collect();
    let filtered_muted_warning_indices: Vec<usize> = muted_web_warning_items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| warning_item_matches_filters(state, item).then_some(idx))
        .collect();
    let muted_state_text = if state.dirty {
        "Muted warnings: pending save"
    } else {
        "Muted warnings: saved"
    };
    ui.horizontal(|ui| {
        if ui.button("Сосчитать").clicked() {
            actions.push(UiLinkEditorAction::PollNow);
        }
        ui.label("Читает текущие значения и заполняет квадраты.");
        ui.label(format!("timeout: {} ms", state.io_timeout_ms));
        ui.label("ALARM: red=hi/lo, yellow=_1, green=normal");
    });
    if web_safe {
        let color = web_summary
            .as_ref()
            .map(|summary| summary.stroke)
            .unwrap_or_else(|| {
                if web_warning_items.is_empty() {
                    egui::Color32::from_rgb(90, 200, 120)
                } else {
                    egui::Color32::from_rgb(255, 201, 94)
                }
            });
        ui.colored_label(
            color,
            if web_warning_items.is_empty() {
                "Web-safe: no obvious web layout issues found"
            } else {
                "Web-safe warnings:"
            },
        );
        if let Some(summary) = &web_summary {
            ui.label(
                egui::RichText::new(&summary.detail)
                    .size(11.0)
                    .color(egui::Color32::from_rgb(171, 187, 204)),
            );
            if !summary.breakdown.is_empty() {
                ui.label(
                    egui::RichText::new(format!("Breakdown: {}", summary.breakdown))
                    .size(11.0)
                    .color(egui::Color32::from_rgb(145, 162, 180)),
                );
            }
            if !summary.fixes.is_empty() {
                ui.label(
                    egui::RichText::new(format!("Suggested fixes: {}", summary.fixes))
                        .size(11.0)
                        .color(egui::Color32::from_rgb(163, 181, 198)),
                );
            }
        }
        if !web_warning_items.is_empty() || muted_warning_count > 0 {
            ui.vertical(|ui| {
                let has_filtered_warnings = !filtered_web_warning_indices.is_empty();
                let has_changed_nav = !state.web_safe_changed_reg_ids.is_empty();

                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("Filters:")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(163, 181, 198)),
                    );
                    ui.checkbox(&mut state.web_safe_filter_labels, "Labels");
                    ui.checkbox(&mut state.web_safe_filter_size, "Size");
                    ui.checkbox(&mut state.web_safe_filter_trend, "Trend");
                    ui.checkbox(&mut state.web_safe_filter_write, "Write");
                    if ui.small_button("All").clicked() {
                        state.web_safe_filter_labels = true;
                        state.web_safe_filter_size = true;
                        state.web_safe_filter_trend = true;
                        state.web_safe_filter_write = true;
                    }
                    if ui.small_button("Clear").clicked() {
                        state.web_safe_filter_labels = false;
                        state.web_safe_filter_size = false;
                        state.web_safe_filter_trend = false;
                        state.web_safe_filter_write = false;
                    }
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!(
                            "showing {} / {}",
                            filtered_web_warning_indices.len(),
                            web_warning_items.len()
                        ))
                        .size(11.0)
                        .color(egui::Color32::from_rgb(145, 162, 180)),
                    );
                    if muted_warning_count > 0 {
                        ui.label(
                            egui::RichText::new(format!(
                                "muted {} ({})",
                                muted_warning_count,
                                if state.dirty { "pending" } else { "saved" }
                            ))
                            .size(11.0)
                            .color(egui::Color32::from_rgb(151, 166, 184)),
                        );
                        ui.checkbox(&mut state.web_safe_show_muted, "Show muted");
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("Warnings:")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(163, 181, 198)),
                    );
                    if ui
                        .add_enabled(
                            selected_warning_reg_id.is_some() && !selected_warning_is_muted,
                            egui::Button::new("Mute selected"),
                        )
                        .clicked()
                    {
                        if let Some(reg_id) = selected_warning_reg_id {
                            state.set_web_safe_muted(reg_id, true);
                        }
                    }
                    if ui
                        .add_enabled(
                            selected_warning_reg_id.is_some() && selected_warning_is_muted,
                            egui::Button::new("Unmute selected"),
                        )
                        .clicked()
                    {
                        if let Some(reg_id) = selected_warning_reg_id {
                            state.set_web_safe_muted(reg_id, false);
                        }
                    }
                    if ui
                        .add_enabled(muted_warning_count > 0, egui::Button::new("Clear muted"))
                        .clicked()
                    {
                        state.web_safe_muted_reg_ids.clear();
                        let mut changed = false;
                        for binding in &mut state.bindings {
                            changed = binding.web_safe_muted || changed;
                            binding.web_safe_muted = false;
                        }
                        if changed {
                            state.dirty = true;
                        }
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_filtered_warnings, egui::Button::new("Prev warning"))
                        .clicked()
                    {
                        focus_warning_reg_id = warning_navigation_target(
                            state,
                            &web_warning_items,
                            &filtered_web_warning_indices,
                            -1,
                        );
                    }
                    if ui
                        .add_enabled(has_filtered_warnings, egui::Button::new("Next warning"))
                        .clicked()
                    {
                        focus_warning_reg_id = warning_navigation_target(
                            state,
                            &web_warning_items,
                            &filtered_web_warning_indices,
                            1,
                        );
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("Reports:")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(163, 181, 198)),
                    );
                    if ui.button("Copy report").clicked() {
                        let report = build_web_safe_report(
                            state,
                            web_summary.as_ref(),
                            &web_warning_items,
                            &muted_web_warning_items,
                        );
                        ui.ctx().copy_text(report);
                        state.status = Some("web-safe report copied to clipboard".to_string());
                        state.err = None;
                    }
                    if ui.button("Export web-safe report").clicked() {
                        match export_web_safe_report(
                            state,
                            web_summary.as_ref(),
                            &web_warning_items,
                            &muted_web_warning_items,
                        ) {
                            Ok(path) => {
                                state.status = Some(format!("web-safe report exported: {}", path));
                                state.err = None;
                            }
                            Err(e) => {
                                state.err = Some(web_safe_action_error("web-safe report export", e));
                            }
                        }
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("Profiles:")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(163, 181, 198)),
                    );
                    if ui.button("Export profile").clicked() {
                        match export_web_safe_profile(state) {
                            Ok(path) => {
                                state.status = Some(web_safe_profile_export_status(&path, None));
                                state.clear_web_safe_profile_preview();
                                state.clear_web_safe_changed_trail();
                                state.err = None;
                            }
                            Err(e) => {
                                state.err = Some(web_safe_action_error("web-safe profile export", e));
                            }
                        }
                    }
                    if ui.button("Preview profile").clicked() {
                        match import_web_safe_profile() {
                            Ok(profile) => {
                                let (preview, will_mute, will_unmute) =
                                    preview_web_safe_profile_diff(
                                        &state.bindings,
                                        &state.web_safe_muted_reg_ids,
                                        state.web_safe_show_muted,
                                        state.web_safe_filter_labels,
                                        state.web_safe_filter_size,
                                        state.web_safe_filter_trend,
                                        state.web_safe_filter_write,
                                        &profile,
                                    );
                                state.set_web_safe_profile_preview(preview, will_mute, will_unmute);
                                state.clear_web_safe_changed_trail();
                                state.status =
                                    Some(web_safe_profile_preview_status(&profile.scope, None));
                                state.err = None;
                            }
                            Err(e) => {
                                state.err = Some(web_safe_action_error("web-safe profile preview", e));
                            }
                        }
                    }
                    if ui.button("Import profile").clicked() {
                        match import_web_safe_profile() {
                            Ok(profile) => {
                                let source_scope = profile.scope.clone();
                                let (preview_text, will_mute, will_unmute) =
                                    preview_web_safe_profile_diff(
                                        &state.bindings,
                                        &state.web_safe_muted_reg_ids,
                                        state.web_safe_show_muted,
                                        state.web_safe_filter_labels,
                                        state.web_safe_filter_size,
                                        state.web_safe_filter_trend,
                                        state.web_safe_filter_write,
                                        &profile,
                                    );
                                let (applied, skipped, muted_changed, filters_changed) =
                                    apply_web_safe_profile(
                                        &mut state.bindings,
                                        &mut state.web_safe_muted_reg_ids,
                                        &mut state.dirty,
                                        &mut state.web_safe_show_muted,
                                        &mut state.web_safe_filter_labels,
                                        &mut state.web_safe_filter_size,
                                        &mut state.web_safe_filter_trend,
                                        &mut state.web_safe_filter_write,
                                        &profile,
                                        true,
                                        true,
                                    );
                                state.set_web_safe_profile_preview(
                                    preview_text,
                                    will_mute,
                                    will_unmute,
                                );
                                state.clear_web_safe_changed_trail();
                                state.status = Some(web_safe_profile_import_status(
                                    "web-safe profile imported",
                                    &source_scope,
                                    Some(applied),
                                    skipped,
                                    None,
                                    Some(muted_changed),
                                    Some(filters_changed),
                                ));
                                state.err = None;
                            }
                            Err(e) => {
                                state.err = Some(web_safe_action_error("web-safe profile import", e));
                            }
                        }
                    }
                    if ui.button("Import mutes only").clicked() {
                        match import_web_safe_profile() {
                            Ok(profile) => {
                                let source_scope = profile.scope.clone();
                                let (preview_text, will_mute, will_unmute) =
                                    preview_web_safe_profile_diff(
                                        &state.bindings,
                                        &state.web_safe_muted_reg_ids,
                                        state.web_safe_show_muted,
                                        state.web_safe_filter_labels,
                                        state.web_safe_filter_size,
                                        state.web_safe_filter_trend,
                                        state.web_safe_filter_write,
                                        &profile,
                                    );
                                let (applied, skipped, muted_changed, _) =
                                    apply_web_safe_profile(
                                        &mut state.bindings,
                                        &mut state.web_safe_muted_reg_ids,
                                        &mut state.dirty,
                                        &mut state.web_safe_show_muted,
                                        &mut state.web_safe_filter_labels,
                                        &mut state.web_safe_filter_size,
                                        &mut state.web_safe_filter_trend,
                                        &mut state.web_safe_filter_write,
                                        &profile,
                                        true,
                                        false,
                                    );
                                state.set_web_safe_profile_preview(
                                    preview_text,
                                    will_mute,
                                    will_unmute,
                                );
                                state.clear_web_safe_changed_trail();
                                state.status = Some(web_safe_profile_import_status(
                                    "web-safe muted profile imported",
                                    &source_scope,
                                    Some(applied),
                                    skipped,
                                    None,
                                    Some(muted_changed),
                                    None,
                                ));
                                state.err = None;
                            }
                            Err(e) => {
                                state.err =
                                    Some(web_safe_action_error("web-safe muted profile import", e));
                            }
                        }
                    }
                    if ui.button("Import filters only").clicked() {
                        match import_web_safe_profile() {
                            Ok(profile) => {
                                let source_scope = profile.scope.clone();
                                let (preview_text, will_mute, will_unmute) =
                                    preview_web_safe_profile_diff(
                                        &state.bindings,
                                        &state.web_safe_muted_reg_ids,
                                        state.web_safe_show_muted,
                                        state.web_safe_filter_labels,
                                        state.web_safe_filter_size,
                                        state.web_safe_filter_trend,
                                        state.web_safe_filter_write,
                                        &profile,
                                    );
                                let (_, skipped, _, filters_changed) =
                                    apply_web_safe_profile(
                                        &mut state.bindings,
                                        &mut state.web_safe_muted_reg_ids,
                                        &mut state.dirty,
                                        &mut state.web_safe_show_muted,
                                        &mut state.web_safe_filter_labels,
                                        &mut state.web_safe_filter_size,
                                        &mut state.web_safe_filter_trend,
                                        &mut state.web_safe_filter_write,
                                        &profile,
                                        false,
                                        true,
                                    );
                                state.set_web_safe_profile_preview(
                                    preview_text,
                                    will_mute,
                                    will_unmute,
                                );
                                state.clear_web_safe_changed_trail();
                                state.status = Some(web_safe_profile_import_status(
                                    "web-safe filter profile imported",
                                    &source_scope,
                                    None,
                                    skipped,
                                    None,
                                    None,
                                    Some(filters_changed),
                                ));
                                state.err = None;
                            }
                            Err(e) => {
                                state.err = Some(web_safe_action_error(
                                    "web-safe filter profile import",
                                    e,
                                ));
                            }
                        }
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("Selection:")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(163, 181, 198)),
                    );
                    if ui
                        .add_enabled(
                            !state.selected_binding_reg_ids.is_empty(),
                            egui::Button::new("Export profile from selection only"),
                        )
                        .clicked()
                    {
                        match export_web_safe_profile_from_selection(state) {
                            Ok(path) => {
                                state.status = Some(web_safe_profile_export_status(
                                    &path,
                                    Some(state.selected_binding_reg_ids.len()),
                                ));
                                state.clear_web_safe_profile_preview();
                                state.clear_web_safe_changed_trail();
                                state.err = None;
                            }
                            Err(e) => {
                                state.err = Some(web_safe_action_error(
                                    "web-safe selection profile export",
                                    e,
                                ));
                            }
                        }
                    }
                    if ui
                        .add_enabled(
                            !state.selected_binding_reg_ids.is_empty(),
                            egui::Button::new("Preview profile into selection only"),
                        )
                        .clicked()
                    {
                        match import_web_safe_profile() {
                            Ok(profile) => {
                                let (preview, will_mute, will_unmute) =
                                    preview_web_safe_profile_diff_for_selection(
                                        &state.bindings,
                                        &state.selected_binding_reg_ids,
                                        &profile,
                                    );
                                state.set_web_safe_profile_preview(preview, will_mute, will_unmute);
                                state.clear_web_safe_changed_trail();
                                state.status = Some(web_safe_profile_preview_status(
                                    &profile.scope,
                                    Some(state.selected_binding_reg_ids.len()),
                                ));
                                state.err = None;
                            }
                            Err(e) => {
                                state.err = Some(web_safe_action_error(
                                    "web-safe selection profile preview",
                                    e,
                                ));
                            }
                        }
                    }
                    if ui
                        .add_enabled(
                            !state.selected_binding_reg_ids.is_empty(),
                            egui::Button::new("Import profile into selection only"),
                        )
                        .clicked()
                    {
                        match import_web_safe_profile() {
                            Ok(profile) => {
                                let source_scope = profile.scope.clone();
                                let (applied, skipped, changed, changed_reg_ids) =
                                    apply_web_safe_profile_to_selection(
                                        &mut state.bindings,
                                        &state.selected_binding_reg_ids,
                                        &mut state.web_safe_muted_reg_ids,
                                        &mut state.dirty,
                                        &profile,
                                    );
                                set_changed_binding_navigation(state, changed_reg_ids);
                                state.status = Some(web_safe_profile_import_status(
                                    "web-safe selection import",
                                    &source_scope,
                                    Some(applied),
                                    skipped,
                                    Some(state.selected_binding_reg_ids.len()),
                                    Some(changed),
                                    None,
                                ));
                                state.err = None;
                            }
                            Err(e) => {
                                state.err = Some(web_safe_action_error(
                                    "web-safe selection profile import",
                                    e,
                                ));
                            }
                        }
                    }
                });

                if has_changed_nav {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new("Changed:")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(163, 181, 198)),
                        );
                        if ui
                            .add_enabled(has_changed_nav, egui::Button::new("Prev changed"))
                            .clicked()
                        {
                            navigate_changed_binding(state, -1);
                        }
                        if ui
                            .add_enabled(has_changed_nav, egui::Button::new("Next changed"))
                            .clicked()
                        {
                            navigate_changed_binding(state, 1);
                        }
                        ui.label(
                            egui::RichText::new(format!(
                                "changed {} / {}",
                                state.web_safe_changed_nav_idx + 1,
                                state.web_safe_changed_reg_ids.len()
                            ))
                            .size(11.0)
                            .color(egui::Color32::from_rgb(164, 189, 214)),
                        );
                        if ui.button("Accept changed").clicked() {
                            let changed_count = state.web_safe_changed_reg_ids.len();
                            clear_changed_binding_navigation(state);
                            state.status = Some(format!(
                                "changed binding trail accepted: {} item(s) reviewed",
                                changed_count
                            ));
                            state.err = None;
                        }
                        if ui.button("Clear changed trail").clicked() {
                            clear_changed_binding_navigation(state);
                        }
                    });
                }
            });
            if let Some(preview) = &state.web_safe_profile_preview {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_unmultiplied(24, 34, 46, 145))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(74, 103, 132)))
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Profile preview")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(186, 208, 230)),
                        );
                        ui.label(
                            egui::RichText::new(preview)
                                .size(11.0)
                                .color(egui::Color32::from_rgb(170, 188, 206)),
                        );
                        if !state.web_safe_profile_preview_add.is_empty()
                            || !state.web_safe_profile_preview_remove.is_empty()
                        {
                            ui.add_space(4.0);
                            ui.horizontal_top(|ui| {
                                if !state.web_safe_profile_preview_add.is_empty() {
                                    egui::Frame::none()
                                        .fill(egui::Color32::from_rgba_unmultiplied(26, 40, 28, 120))
                                        .stroke(egui::Stroke::new(
                                            1.0,
                                            egui::Color32::from_rgb(74, 126, 82),
                                        ))
                                        .rounding(egui::Rounding::same(8.0))
                                        .inner_margin(egui::Margin::same(6.0))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new("Will mute")
                                                    .size(11.0)
                                                    .color(egui::Color32::from_rgb(184, 220, 189)),
                                            );
                                            egui::ScrollArea::vertical()
                                                .max_height(64.0)
                                                .show(ui, |ui| {
                                                    for item in &state.web_safe_profile_preview_add {
                                                        ui.label(item);
                                                    }
                                                });
                                        });
                                }
                                if !state.web_safe_profile_preview_remove.is_empty() {
                                    egui::Frame::none()
                                        .fill(egui::Color32::from_rgba_unmultiplied(46, 32, 24, 120))
                                        .stroke(egui::Stroke::new(
                                            1.0,
                                            egui::Color32::from_rgb(156, 118, 86),
                                        ))
                                        .rounding(egui::Rounding::same(8.0))
                                        .inner_margin(egui::Margin::same(6.0))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new("Will unmute")
                                                    .size(11.0)
                                                    .color(egui::Color32::from_rgb(231, 206, 188)),
                                            );
                                            egui::ScrollArea::vertical()
                                                .max_height(64.0)
                                                .show(ui, |ui| {
                                                    for item in &state.web_safe_profile_preview_remove {
                                                        ui.label(item);
                                                    }
                                                });
                                        });
                                }
                            });
                        }
                    });
            }
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(50, 40, 24, 112))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(166, 129, 78)))
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::same(8.0))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "Active warnings: {} shown / {} total",
                                filtered_web_warning_indices.len(),
                                web_warning_items.len()
                            ))
                            .size(11.0)
                            .color(egui::Color32::from_rgb(242, 216, 168)),
                        );
                        if web_warning_items.is_empty() && muted_warning_count > 0 {
                            ui.label(
                                egui::RichText::new("All current web-safe warnings are muted.")
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(208, 188, 148)),
                            );
                        }
                    });
                    egui::ScrollArea::vertical().max_height(72.0).show(ui, |ui| {
                        if filtered_web_warning_indices.is_empty() && !web_warning_items.is_empty() {
                            ui.label(
                                egui::RichText::new("No active warnings match the current filters.")
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(190, 174, 146)),
                            );
                        } else {
                            for idx in &filtered_web_warning_indices {
                                let item = &web_warning_items[*idx];
                                let is_selected = state.selected_binding_reg_id == Some(item.reg_id)
                                    || state.selected_binding_reg_ids.contains(&item.reg_id);
                                let row = render_warning_row(
                                    ui,
                                    WebSafeWarningRow {
                                        message: &item.message,
                                        labels: item.labels,
                                        size: item.size,
                                        trend: item.trend,
                                        write: item.write,
                                        selected: is_selected,
                                        muted: false,
                                    },
                                );
                                if row.clicked() {
                                    focus_warning_reg_id = Some(item.reg_id);
                                }
                            }
                        }
                    });
                });
            if muted_warning_count > 0 {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_unmultiplied(24, 30, 38, 108))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(84, 96, 110)))
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(muted_state_text)
                                .size(11.0)
                                .color(egui::Color32::from_rgb(163, 181, 198)),
                        );
                        if state.web_safe_show_muted && !muted_web_warning_items.is_empty() {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "Muted warnings: {} shown / {} total",
                                    filtered_muted_warning_indices.len(),
                                    muted_web_warning_items.len()
                                ))
                                .size(11.0)
                                .color(egui::Color32::from_rgb(173, 189, 206)),
                            );
                            egui::ScrollArea::vertical().max_height(64.0).show(ui, |ui| {
                                if filtered_muted_warning_indices.is_empty() {
                                    ui.label(
                                        egui::RichText::new(
                                            "No muted warnings match the current filters.",
                                        )
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(144, 158, 173)),
                                    );
                                } else {
                                    for idx in &filtered_muted_warning_indices {
                                        let item = &muted_web_warning_items[*idx];
                                        let is_selected = state.selected_binding_reg_id
                                            == Some(item.reg_id)
                                            || state.selected_binding_reg_ids.contains(&item.reg_id);
                                        let row = render_warning_row(
                                            ui,
                                            WebSafeWarningRow {
                                                message: &item.message,
                                                labels: item.labels,
                                                size: item.size,
                                                trend: item.trend,
                                                write: item.write,
                                                selected: is_selected,
                                                muted: true,
                                            },
                                        );
                                        if row.clicked() {
                                            focus_warning_reg_id = Some(item.reg_id);
                                        }
                                    }
                                }
                            });
                        } else {
                            ui.label(
                                egui::RichText::new(
                                    "Muted warnings are hidden from the list. Enable 'Show muted' above to review them.",
                                )
                                .size(11.0)
                                .color(egui::Color32::from_rgb(144, 158, 173)),
                            );
                        }
                    });
            }
        }
    }
    if let Some(reg_id) = focus_warning_reg_id {
        focus_binding_from_warning(state, reg_id);
    }
    let (source_text, source_fill, source_stroke) = if state.err.is_some() {
        (
            "Source: error",
            egui::Color32::from_rgb(61, 30, 37),
            egui::Color32::from_rgb(140, 60, 75),
        )
    } else if !state.last_cmd_result.is_empty() {
        (
            "Source: write",
            egui::Color32::from_rgb(24, 50, 37),
            egui::Color32::from_rgb(47, 122, 83),
        )
    } else if state.live_values.values().any(|v| v.is_some()) {
        (
            "Source: live",
            egui::Color32::from_rgb(24, 50, 37),
            egui::Color32::from_rgb(47, 122, 83),
        )
    } else {
        (
            "Source: idle",
            egui::Color32::from_rgb(18, 25, 38),
            egui::Color32::from_rgb(50, 64, 85),
        )
    };
    let web_safe_chip = if let Some(summary) = &web_summary {
        (
            format!("Web-safe: {}", summary.label),
            summary.fill,
            summary.stroke,
        )
    } else {
        (
            "Web-safe: off".to_string(),
            egui::Color32::from_rgb(18, 25, 38),
            egui::Color32::from_rgb(50, 64, 85),
        )
    };
    let preview_status_text = if let Some(err) = &state.err {
        format!("Preview error: {err}")
    } else if let Some(status) = &state.status {
        status.clone()
    } else if web_safe && !web_warning_items.is_empty() {
        format!(
            "{} web-safe issue(s) highlighted on tiles. {}{}",
            web_warning_items.len(),
            web_summary
                .as_ref()
                .map(|summary| summary.detail.as_str())
                .unwrap_or("Review warnings above."),
            web_summary
                .as_ref()
                .filter(|summary| !summary.fixes.is_empty())
                .map(|summary| format!(" Suggested fixes: {}.", summary.fixes))
                .unwrap_or_default()
        )
    } else if web_safe {
        web_summary
            .as_ref()
            .map(|summary| format!(
                "Web-safe preview matches the current ss6 card layout. {}",
                summary.detail
            ))
            .unwrap_or_else(|| "Web-safe preview matches the current ss6 card layout.".to_string())
    } else {
        "Preview is ready. Click Poll Now to refresh live values.".to_string()
    };
    let draw_chip = |ui: &mut egui::Ui,
                     text: &str,
                     fill: egui::Color32,
                     stroke: egui::Color32| {
        egui::Frame::none()
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, stroke))
            .rounding(egui::Rounding::same(255.0))
            .inner_margin(egui::Margin::symmetric(10.0, 5.0))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(text)
                        .size(11.0)
                        .color(egui::Color32::from_rgb(230, 239, 248)),
                );
            });
    };
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(23, 33, 48))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(42, 54, 72)))
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::same(10.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                draw_chip(
                    ui,
                    "Mode: Preview",
                    egui::Color32::from_rgb(24, 49, 72),
                    egui::Color32::from_rgb(49, 100, 141),
                );
                draw_chip(ui, source_text, source_fill, source_stroke);
                draw_chip(ui, web_safe_chip.0.as_str(), web_safe_chip.1, web_safe_chip.2);
                if muted_warning_count > 0 {
                    draw_chip(
                        ui,
                        format!(
                            "Muted: {} {}",
                            muted_warning_count,
                            if state.dirty { "pending" } else { "saved" }
                        )
                        .as_str(),
                        egui::Color32::from_rgb(24, 31, 41),
                        egui::Color32::from_rgb(76, 92, 112),
                    );
                }
                if let Some(summary) = &web_summary {
                    if summary.labels > 0 {
                        draw_chip(
                            ui,
                            format!("Labels: {}", summary.labels).as_str(),
                            egui::Color32::from_rgb(45, 38, 22),
                            egui::Color32::from_rgb(156, 120, 55),
                        );
                    }
                    if summary.size > 0 {
                        draw_chip(
                            ui,
                            format!("Size: {}", summary.size).as_str(),
                            egui::Color32::from_rgb(44, 34, 25),
                            egui::Color32::from_rgb(150, 106, 66),
                        );
                    }
                    if summary.trend > 0 {
                        draw_chip(
                            ui,
                            format!("Trend: {}", summary.trend).as_str(),
                            egui::Color32::from_rgb(34, 42, 26),
                            egui::Color32::from_rgb(96, 142, 73),
                        );
                    }
                    if summary.write > 0 {
                        draw_chip(
                            ui,
                            format!("Write: {}", summary.write).as_str(),
                            egui::Color32::from_rgb(48, 32, 34),
                            egui::Color32::from_rgb(155, 88, 98),
                        );
                    }
                }
            });
            ui.add_space(6.0);
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(10, 16, 24, 148))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(36, 50, 68)))
                .rounding(egui::Rounding::same(10.0))
                .inner_margin(egui::Margin::same(8.0))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(preview_status_text)
                            .size(12.0)
                            .color(egui::Color32::from_rgb(210, 222, 234)),
                    );
                });
        });
    let mut max_x = 0.0_f32;
    let mut max_y = 0.0_f32;
    let mut needs_external_gutter = false;
    let bindings_snapshot = state.bindings.clone();
    for b in &bindings_snapshot {
        if !b.visible {
            continue;
        }
        let is_bool = b.reg_tip == 0 && b.reg_bits.is_some();
        let min_w = if is_bool { 8 } else { 30 };
        let min_h = if is_bool { 8 } else { 18 };
        let bw = b.w.max(min_w) as f32;
        let bh = b.h.max(min_h) as f32;
        max_x = max_x.max(b.x as f32 + bw);
        max_y = max_y.max(b.y as f32 + bh);
        if web_safe && web_safe_uses_external_label(b) {
            needs_external_gutter = true;
        }
    }
    let origin_x = if web_safe && needs_external_gutter { 140.0 } else { 0.0 };
    let origin_y = 0.0_f32;
    let content_size = egui::vec2(
        (origin_x + max_x + 20.0).max(620.0),
        (origin_y + max_y + 20.0).max(460.0),
    );
    let avail = ui.available_size_before_wrap();
    let viewport = egui::vec2(avail.x.max(320.0), avail.y.max(240.0));
    let fit_scale = (viewport.x / content_size.x)
        .min(viewport.y / content_size.y)
        .clamp(0.35, 3.0);
    let canvas_size = egui::vec2(content_size.x * fit_scale, content_size.y * fit_scale);
    let (rect, resp) = ui.allocate_exact_size(canvas_size, egui::Sense::click());
    let painter = ui.painter_at(rect);
    if web_safe {
        painter.rect_filled(rect, 8.0, egui::Color32::from_rgb(18, 24, 33));
        painter.rect_stroke(
            rect,
            8.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 61, 80)),
        );
    } else {
        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(16, 18, 30));
        painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, egui::Color32::from_gray(80)));
    }

    let mut bindings_snapshot = state.bindings.clone();
    // Draw image items first so technical schemes behave like a background layer.
    bindings_snapshot.sort_by_key(|b| (if is_image_item(b) { 0 } else { 1 }, b.pos, b.reg_id));
    for b in &bindings_snapshot {
                if !b.visible {
                    continue;
                }
                let is_text = b.is_text || b.reg_id <= 0;
                let is_bool = b.reg_tip == 0 && b.reg_bits.is_some();
                let min_w = if is_bool { 8 } else { 30 };
                let min_h = if is_bool { 8 } else { 18 };
                let bw = b.w.max(min_w) as f32;
                let bh = b.h.max(min_h) as f32;
                let top_left = rect.min
                    + egui::vec2((origin_x + b.x as f32) * fit_scale, (origin_y + b.y as f32) * fit_scale);
                let r = egui::Rect::from_min_size(top_left, egui::vec2(bw * fit_scale, bh * fit_scale));
                let is_tu = b.reg_n_mb == 1 || b.reg_tip == 1;
                let live = *state.live_values.get(&b.reg_id).unwrap_or(&None);
                let component_kind = preview_component_kind(b);
                let is_image = is_image_item(b);
                let label_text = binding_display_label(b);
                let external_label = web_safe && web_safe_uses_external_label(b);
                let internal_label = web_safe && web_safe_prefers_internal_label(b);
                let has_web_warning = web_safe && binding_has_active_web_warning(state, b);
                let has_muted_web_warning = web_safe && binding_has_muted_web_warning(state, b);
                let selected_binding = state.selected_binding_reg_id == Some(b.reg_id)
                    || state.selected_binding_reg_ids.contains(&b.reg_id);
                let alarm_color = preview_alarm_color(
                    live,
                    state.alarm_rules_by_reg.get(&b.reg_id),
                    egui::Color32::from_rgb(58, 88, 164),
                );
                if is_image {
                    let alpha = (b.scale_max.unwrap_or(1.0).clamp(0.0, 1.0) * 255.0) as u8;
                    let cache_version = image_cache_version(b);
                    let entry = load_preview_image_texture(ui.ctx(), state, &label_text, &cache_version);
                    if let Some(texture) = entry.texture {
                        painter.rect_filled(r, if web_safe { 8.0 } else { 2.0 }, egui::Color32::from_rgb(8, 12, 18));
                        let fit_rect = image_fit_rect(r, texture.size_vec2(), b.fmt.as_deref().unwrap_or("contain"));
                        let clipped = painter.with_clip_rect(r);
                        clipped.image(
                            texture.id(),
                            fit_rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                        );
                        painter.rect_stroke(
                            r,
                            if web_safe { 8.0 } else { 2.0 },
                            egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 130, 180)),
                        );
                    } else {
                        painter.rect_filled(
                            r,
                            if web_safe { 8.0 } else { 2.0 },
                            egui::Color32::from_rgba_unmultiplied(19, 28, 42, alpha.max(40)),
                        );
                        painter.rect_stroke(
                            r,
                            if web_safe { 8.0 } else { 2.0 },
                            egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 130, 180)),
                        );
                        let diag = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(110, 170, 220, 90));
                        painter.line_segment([r.left_top(), r.right_bottom()], diag);
                        painter.line_segment([r.right_top(), r.left_bottom()], diag);
                    }
                } else if is_text {
                    painter.rect_filled(
                        r,
                        if web_safe { 6.0 } else { 0.0 },
                        egui::Color32::from_rgb(16, 18, 30),
                    );
                } else if is_tu {
                    let mid_x = (r.min.x + r.max.x) * 0.5;
                    let left = egui::Rect::from_min_max(r.min, egui::pos2(mid_x, r.max.y));
                    let right = egui::Rect::from_min_max(egui::pos2(mid_x, r.min.y), r.max);
                    painter.rect_filled(left, 3.0, egui::Color32::from_rgb(170, 48, 48));
                    painter.rect_filled(right, 3.0, egui::Color32::from_rgb(48, 150, 64));
                } else if component_kind == "bar" {
                    let scale_max = preview_scale_max(b);
                    let v = live.unwrap_or(0.0).clamp(0.0, scale_max);
                    let ratio = (v / scale_max) as f32;
                    let fill_color = preview_alarm_color(
                        live,
                        state.alarm_rules_by_reg.get(&b.reg_id),
                        egui::Color32::from_rgb(48, 120, 180),
                    );
                    if web_safe {
                        painter.rect_filled(r, 8.0, egui::Color32::from_rgb(27, 35, 49));
                        let track = egui::Rect::from_min_max(
                            egui::pos2(r.min.x + 4.0, r.min.y + 22.0),
                            egui::pos2(r.max.x - 4.0, r.max.y - 8.0),
                        );
                        painter.rect_filled(
                            track,
                            7.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 16),
                        );
                        let fill_w = (track.width() * ratio).max(0.0);
                        if fill_w > 0.5 {
                            let fill_r = egui::Rect::from_min_size(
                                track.min,
                                egui::vec2(fill_w.max(if ratio > 0.0 { 10.0 } else { 0.0 }), track.height()),
                            );
                            painter.rect_filled(fill_r, 7.0, fill_color);
                        }
                        draw_bar_alarm_markers(
                            &painter,
                            track,
                            scale_max,
                            state.alarm_rules_by_reg.get(&b.reg_id),
                        );
                    } else {
                        painter.rect_filled(r, 3.0, egui::Color32::from_rgb(28, 32, 44));
                    }
                    let is_vertical = r.height() > r.width();
                    if !web_safe && is_vertical {
                        let fill_h = (r.height() * ratio).max(0.0);
                        if fill_h > 0.5 {
                            let fill_r = egui::Rect::from_min_max(
                                egui::pos2(r.min.x, r.max.y - fill_h),
                                r.max,
                            );
                            painter.rect_filled(fill_r, 3.0, fill_color);
                        }
                    } else if !web_safe {
                        let fill_w = (r.width() * ratio).max(0.0);
                        if fill_w > 0.5 {
                            let fill_r = egui::Rect::from_min_size(r.min, egui::vec2(fill_w, r.height()));
                            painter.rect_filled(fill_r, 3.0, fill_color);
                        }
                    }
                    if !web_safe {
                        draw_bar_alarm_markers(
                            &painter,
                            r.shrink(1.0),
                            scale_max,
                            state.alarm_rules_by_reg.get(&b.reg_id),
                        );
                    }
                    painter.rect_stroke(
                        r,
                        if web_safe { 8.0 } else { 3.0 },
                        egui::Stroke::new(1.0, fill_color.gamma_multiply(0.85)),
                    );
                } else if component_kind == "gauge" {
                    painter.rect_filled(
                        r,
                        if web_safe { 10.0 } else { 6.0 },
                        egui::Color32::from_rgb(20, 24, 34),
                    );
                    let side_pad = 8.0;
                    let top_pad = if web_safe { 12.0 } else { 8.0 };
                    let gauge_height = (r.height() * 0.48).max(16.0);
                    let center = egui::pos2(r.center().x, r.min.y + top_pad + gauge_height);
                    let radius = ((r.width() * 0.5 - side_pad).min(gauge_height)).max(8.0);
                    let scale_max = preview_scale_max(b);
                    let start_angle = std::f32::consts::PI;
                    let end_angle = std::f32::consts::TAU;
                    let steps = 28;
                    let mut pts = Vec::with_capacity(steps + 1);
                    for i in 0..=steps {
                        let t = i as f32 / steps as f32;
                        let a = start_angle + (end_angle - start_angle) * t;
                        pts.push(center + egui::vec2(a.cos() * radius, a.sin() * radius));
                    }
                    painter.add(egui::Shape::line(
                        pts,
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(90, 110, 140)),
                    ));
                    draw_gauge_alarm_markers(
                        &painter,
                        center,
                        radius,
                        scale_max,
                        state.alarm_rules_by_reg.get(&b.reg_id),
                    );
                    let ratio = (live.unwrap_or(0.0).clamp(0.0, scale_max) / scale_max) as f32;
                    let needle_angle = start_angle + (end_angle - start_angle) * ratio;
                    let needle_tip = center
                        + egui::vec2(needle_angle.cos() * radius * 0.88, needle_angle.sin() * radius * 0.88);
                    painter.line_segment(
                        [center, needle_tip],
                        egui::Stroke::new(2.5, preview_alarm_color(live, state.alarm_rules_by_reg.get(&b.reg_id), egui::Color32::from_rgb(80, 200, 220))),
                    );
                    painter.circle_filled(center, 3.5, egui::Color32::WHITE);
                } else if component_kind == "button" {
                    draw_button_tile(&painter, r, web_safe, b.writable);
                } else if component_kind == "led" {
                    draw_led_tile(
                        &painter,
                        r,
                        web_safe,
                        live,
                        state.alarm_rules_by_reg.get(&b.reg_id),
                    );
                } else if component_kind == "numeric" {
                    draw_numeric_tile(&painter, r, web_safe);
                } else if component_kind == "setpoint" {
                    let panel_color = preview_alarm_color(
                        live,
                        state.alarm_rules_by_reg.get(&b.reg_id),
                        egui::Color32::from_rgb(32, 40, 58),
                    )
                    .gamma_multiply(0.28);
                    painter.rect_filled(r, if web_safe { 10.0 } else { 5.0 }, panel_color);
                    let accent = egui::Rect::from_min_max(r.min, egui::pos2(r.min.x + 6.0, r.max.y));
                    painter.rect_filled(
                        accent,
                        5.0,
                        preview_alarm_color(
                            live,
                            state.alarm_rules_by_reg.get(&b.reg_id),
                            egui::Color32::from_rgb(80, 170, 220),
                        ),
                    );
                    let header = egui::Rect::from_min_max(
                        egui::pos2(r.min.x + 8.0, r.min.y + 4.0),
                        egui::pos2(r.max.x - 6.0, r.min.y + 14.0),
                    );
                    painter.rect_filled(header, 3.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12));
                } else if component_kind == "trend" {
                    draw_trend_tile(
                        &painter,
                        r,
                        web_safe,
                        state.trend_history.get(&b.reg_id),
                        live,
                        state.alarm_rules_by_reg.get(&b.reg_id),
                    );
                } else {
                    let fill = if is_bool {
                        egui::Color32::from_rgb(58, 88, 164)
                    } else {
                        alarm_color
                    };
                    painter.rect_filled(r, 3.0, fill);
                }
                if !is_text && component_kind != "led" && component_kind != "bar" && component_kind != "trend" {
                    painter.rect_stroke(
                        r,
                        if web_safe { 10.0 } else { 3.0 },
                        egui::Stroke::new(
                            1.0,
                            if web_safe {
                                egui::Color32::from_rgb(50, 64, 82)
                            } else {
                                egui::Color32::WHITE
                            },
                        ),
                    );
                }

                let lead = label_text;
                if !is_text && (!web_safe || external_label) {
                    painter.text(
                        r.left_center() + egui::vec2(-6.0, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        lead,
                        egui::TextStyle::Body.resolve(ui.style()),
                        egui::Color32::LIGHT_GRAY,
                    );
                }
                let is_bool = b.reg_tip == 0 && b.reg_bits.is_some();
                let is_u16_holding = !is_tu && !is_bool && !matches!(b.reg_tip, 2 | 4 | 5);
                if is_image {
                    let cache_version = image_cache_version(b);
                    let entry = load_preview_image_texture(ui.ctx(), state, &lead, &cache_version);
                    if entry.texture.is_some() {
                        let chip = egui::Rect::from_min_size(r.min + egui::vec2(6.0, 6.0), egui::vec2((r.width() - 12.0).min(220.0), 20.0));
                        painter.rect_filled(chip, 5.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 110));
                        painter.text(
                            chip.left_center() + egui::vec2(6.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            lead,
                            egui::TextStyle::Small.resolve(ui.style()),
                            egui::Color32::from_rgb(220, 235, 245),
                        );
                    } else {
                        painter.text(
                            egui::pos2(r.center().x, r.center().y - 10.0),
                            egui::Align2::CENTER_CENTER,
                            "IMAGE",
                            egui::TextStyle::Button.resolve(ui.style()),
                            egui::Color32::from_rgb(220, 238, 255),
                        );
                        painter.text(
                            egui::pos2(r.center().x, r.center().y + 10.0),
                            egui::Align2::CENTER_CENTER,
                            entry.error.unwrap_or_else(|| lead.to_string()),
                            egui::TextStyle::Small.resolve(ui.style()),
                            egui::Color32::from_rgb(180, 205, 225),
                        );
                    }
                } else if is_text {
                    painter.text(
                        r.center(),
                        egui::Align2::CENTER_CENTER,
                        lead,
                        egui::TextStyle::Body.resolve(ui.style()),
                        egui::Color32::WHITE,
                    );
                } else if is_tu {
                    painter.text(
                        r.center(),
                        egui::Align2::CENTER_CENTER,
                        "OFF | ON",
                        egui::TextStyle::Body.resolve(ui.style()),
                        egui::Color32::WHITE,
                    );
                } else if component_kind == "led" {
                    let status_text = if live.unwrap_or(0.0) >= 0.5 { "ON" } else { "OFF" };
                    let font_id = if r.width().min(r.height()) < 26.0 {
                        egui::TextStyle::Small.resolve(ui.style())
                    } else {
                        egui::TextStyle::Body.resolve(ui.style())
                    };
                    if internal_label {
                        painter.text(
                            egui::pos2(r.center().x, r.min.y + 6.0),
                            egui::Align2::CENTER_TOP,
                            lead,
                            egui::TextStyle::Small.resolve(ui.style()),
                            egui::Color32::from_rgb(207, 217, 230),
                        );
                    }
                    painter.text(
                        if web_safe {
                            egui::pos2(r.center().x, r.max.y - 10.0)
                        } else {
                            r.center()
                        },
                        egui::Align2::CENTER_CENTER,
                        status_text,
                        font_id,
                        egui::Color32::WHITE,
                    );
                } else if is_bool {
                    let on = live.unwrap_or(0.0) >= 0.5;
                    let fill = if on {
                        egui::Color32::from_rgb(40, 190, 70)
                    } else {
                        egui::Color32::BLACK
                    };
                    painter.rect_filled(r, 3.0, fill);
                    painter.rect_stroke(r, 3.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                } else if component_kind == "bar" {
                    let cur_text = fmt_binding_live(b, live, false);
                    let font_id = if r.height() > r.width() {
                        egui::TextStyle::Small.resolve(ui.style())
                    } else {
                        egui::TextStyle::Body.resolve(ui.style())
                    };
                    if internal_label {
                        painter.text(
                            egui::pos2(r.min.x + 8.0, r.min.y + 6.0),
                            egui::Align2::LEFT_TOP,
                            lead,
                            egui::TextStyle::Small.resolve(ui.style()),
                            egui::Color32::from_rgb(207, 217, 230),
                        );
                    }
                    painter.text(
                        if web_safe {
                            egui::pos2(r.center().x, r.max.y - 10.0)
                        } else {
                            r.center()
                        },
                        if web_safe {
                            egui::Align2::CENTER_BOTTOM
                        } else {
                            egui::Align2::CENTER_CENTER
                        },
                        cur_text,
                        font_id,
                        egui::Color32::WHITE,
                    );
                } else if component_kind == "gauge" {
                    let val_text = fmt_binding_live(b, live, false);
                    if internal_label {
                        painter.text(
                            egui::pos2(r.center().x, r.min.y + 6.0),
                            egui::Align2::CENTER_TOP,
                            lead,
                            egui::TextStyle::Small.resolve(ui.style()),
                            egui::Color32::from_rgb(207, 217, 230),
                        );
                    }
                    painter.text(
                        egui::pos2(r.center().x, r.center().y + r.height() * 0.18),
                        egui::Align2::CENTER_CENTER,
                        val_text,
                        egui::TextStyle::Body.resolve(ui.style()),
                        egui::Color32::WHITE,
                    );
                } else if component_kind == "button" {
                    painter.text(
                        egui::pos2(r.center().x, r.min.y + 6.0),
                        egui::Align2::CENTER_TOP,
                        if b.writable { "Write action" } else { "Button" },
                        egui::TextStyle::Small.resolve(ui.style()),
                        egui::Color32::from_rgb(233, 243, 255),
                    );
                    painter.text(
                        egui::pos2(r.center().x, r.center().y - 4.0),
                        egui::Align2::CENTER_CENTER,
                        lead,
                        egui::TextStyle::Button.resolve(ui.style()),
                        egui::Color32::WHITE,
                    );
                    painter.text(
                        egui::pos2(r.center().x, r.center().y + 10.0),
                        egui::Align2::CENTER_CENTER,
                        if b.writable { "Click to send" } else { "Read-only" },
                        egui::TextStyle::Small.resolve(ui.style()),
                        if b.writable {
                            egui::Color32::from_rgb(170, 220, 255)
                        } else {
                            egui::Color32::LIGHT_GRAY
                        },
                    );
                } else if component_kind == "numeric" {
                    let v = fmt_binding_live(b, live, is_u16_holding);
                    if internal_label {
                        painter.text(
                            egui::pos2(r.center().x, r.min.y + 6.0),
                            egui::Align2::CENTER_TOP,
                            lead,
                            egui::TextStyle::Small.resolve(ui.style()),
                            egui::Color32::from_rgb(207, 217, 230),
                        );
                    }
                    painter.text(
                        if web_safe {
                            egui::pos2(r.center().x, r.center().y + 4.0)
                        } else {
                            r.center()
                        },
                        egui::Align2::CENTER_CENTER,
                        v,
                        egui::TextStyle::Heading.resolve(ui.style()),
                        egui::Color32::WHITE,
                    );
                } else if component_kind == "setpoint" {
                    let val_text = fmt_binding_live(b, live, is_u16_holding);
                    if internal_label {
                        painter.text(
                            egui::pos2(r.center().x, r.min.y + 6.0),
                            egui::Align2::CENTER_TOP,
                            lead,
                            egui::TextStyle::Small.resolve(ui.style()),
                            egui::Color32::from_rgb(207, 217, 230),
                        );
                    }
                    painter.text(
                        egui::pos2(r.center().x, r.center().y - 4.0),
                        egui::Align2::CENTER_CENTER,
                        val_text,
                        egui::TextStyle::Heading.resolve(ui.style()),
                        egui::Color32::WHITE,
                    );
                    painter.text(
                        egui::pos2(r.center().x, r.center().y + 10.0),
                        egui::Align2::CENTER_CENTER,
                        if b.writable { "Setpoint" } else { "Read-only" },
                        egui::TextStyle::Small.resolve(ui.style()),
                        if b.writable {
                            egui::Color32::from_rgb(150, 220, 255)
                        } else {
                            egui::Color32::LIGHT_GRAY
                        },
                    );
                } else if component_kind == "trend" {
                    let val_text = fmt_binding_live(b, live, false);
                    if internal_label {
                        painter.text(
                            egui::pos2(r.center().x, r.min.y + 6.0),
                            egui::Align2::CENTER_TOP,
                            lead,
                            egui::TextStyle::Small.resolve(ui.style()),
                            egui::Color32::from_rgb(207, 217, 230),
                        );
                    }
                    painter.text(
                        egui::pos2(r.center().x, r.max.y - 10.0),
                        egui::Align2::CENTER_CENTER,
                        val_text,
                        egui::TextStyle::Small.resolve(ui.style()),
                        egui::Color32::WHITE,
                    );
                } else {
                    let is_edit = b.writable && state.preview_edit_reg_id == Some(b.reg_id);
                    if is_edit {
                        let e = state
                            .cmd_inputs
                            .entry(b.reg_id)
                            .or_insert_with(|| {
                                if is_u16_holding {
                                    live.map(|v| format!("{}", v.round().clamp(0.0, 65535.0) as i64))
                                        .unwrap_or_default()
                                } else {
                                    live.map(|v| v.to_string()).unwrap_or_default()
                                }
                            });
                        let edit_rect = r.shrink(2.0);
                        let inner = ui.scope_builder(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
                            ui.add(egui::TextEdit::singleline(e).desired_width(edit_rect.width() - 4.0))
                        });
                        let resp_edit = inner.inner;
                        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if resp_edit.lost_focus() && enter {
                            let parsed = if is_u16_holding {
                                e.trim()
                                    .parse::<i64>()
                                    .ok()
                                    .map(|v| v.clamp(0, 65535) as f64)
                            } else {
                                e.trim().replace(',', ".").parse::<f64>().ok()
                            };
                            if let Some(v) = parsed {
                                actions.push(UiLinkEditorAction::WriteValue { reg_id: b.reg_id, val: v });
                            } else {
                                state.err = Some(format!("bad value for reg {}", b.reg_id));
                            }
                            state.preview_edit_reg_id = None;
                        }
                    } else {
                        let v = fmt_binding_live(b, live, is_u16_holding);
                        let mark = if b.writable { " [wr]" } else { "" };
                        if internal_label {
                            painter.text(
                                egui::pos2(r.center().x, r.min.y + 6.0),
                                egui::Align2::CENTER_TOP,
                                lead,
                                egui::TextStyle::Small.resolve(ui.style()),
                                egui::Color32::from_rgb(207, 217, 230),
                            );
                        }
                        painter.text(
                            r.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}{}", v, mark),
                            egui::TextStyle::Body.resolve(ui.style()),
                            egui::Color32::WHITE,
                        );
                    }
                }
                if has_web_warning {
                    draw_web_safe_warning_badge(&painter, r);
                } else if has_muted_web_warning {
                    draw_web_safe_muted_badge(&painter, r);
                }
                if selected_binding {
                    painter.rect_stroke(
                        r.expand(if web_safe { 3.0 } else { 2.0 }),
                        if web_safe { 10.0 } else { 6.0 },
                        egui::Stroke::new(
                            if web_safe { 2.5 } else { 2.0 },
                            egui::Color32::from_rgb(120, 196, 255),
                        ),
                    );
                }
                if let Some(mark) = state.last_cmd_result.get(&b.reg_id) {
                    let color = if mark.starts_with("OK") {
                        egui::Color32::from_rgb(80, 220, 120)
                    } else {
                        egui::Color32::from_rgb(240, 90, 90)
                    };
                    painter.text(
                        r.right_top() + egui::vec2(-4.0, 2.0),
                        egui::Align2::RIGHT_TOP,
                        mark,
                        egui::TextStyle::Small.resolve(ui.style()),
                        color,
                    );
                }
            }
    if resp.clicked_by(egui::PointerButton::Primary) {
                if let Some(pos) = resp.interact_pointer_pos() {
                    for b in state.bindings.iter().rev() {
                        if !b.visible {
                            continue;
                        }
                        if b.is_text || b.reg_id <= 0 {
                            continue;
                        }
                        let bw = b.w.max(30) as f32;
                        let bh = b.h.max(18) as f32;
                        let top_left = rect.min
                            + egui::vec2((origin_x + b.x as f32) * fit_scale, (origin_y + b.y as f32) * fit_scale);
                        let r = egui::Rect::from_min_size(top_left, egui::vec2(bw * fit_scale, bh * fit_scale));
                        if r.contains(pos) {
                            let is_tu = b.reg_n_mb == 1 || b.reg_tip == 1;
                            let is_bool = b.reg_tip == 0 && b.reg_bits.is_some();
                            let is_word16 = !is_tu && !is_bool && !matches!(b.reg_tip, 2 | 4 | 5);
                            let component_kind = preview_component_kind(b);
                            if is_tu {
                                let on = pos.x >= r.center().x;
                                actions.push(UiLinkEditorAction::SendTu { reg_id: b.reg_id, on });
                            } else if component_kind == "button" && b.writable {
                                actions.push(UiLinkEditorAction::WriteValue {
                                    reg_id: b.reg_id,
                                    val: 1.0,
                                });
                            } else if b.writable {
                                if is_bool {
                                    let cur = state
                                        .live_values
                                        .get(&b.reg_id)
                                        .copied()
                                        .flatten()
                                        .unwrap_or(0.0);
                                    let next = if cur >= 0.5 { 0.0 } else { 1.0 };
                                    actions.push(UiLinkEditorAction::WriteValue {
                                        reg_id: b.reg_id,
                                        val: next,
                                    });
                                } else if is_word16 {
                                    state.preview_edit_reg_id = Some(b.reg_id);
                                    state
                                        .cmd_inputs
                                        .entry(b.reg_id)
                                        .or_insert_with(|| {
                                            state
                                                .live_values
                                                .get(&b.reg_id)
                                                .copied()
                                                .flatten()
                                                .map(|v| v.to_string())
                                                .unwrap_or_default()
                                        });
                                    state.status = Some(format!("reg {}: direct edit mode", b.reg_id));
                                } else {
                                    state.preview_edit_reg_id = Some(b.reg_id);
                                    state
                                        .cmd_inputs
                                        .entry(b.reg_id)
                                        .or_insert_with(|| {
                                            state
                                                .live_values
                                                .get(&b.reg_id)
                                                .copied()
                                                .flatten()
                                                .map(|v| v.to_string())
                                                .unwrap_or_default()
                                        });
                                }
                            } else {
                                state.preview_edit_reg_id = None;
                            }
                            break;
                        }
                    }
                }
            }
    if resp.clicked_by(egui::PointerButton::Secondary) {
                if let Some(pos) = resp.interact_pointer_pos() {
                    for b in state.bindings.iter().rev() {
                        if !b.visible {
                            continue;
                        }
                        if b.is_text || b.reg_id <= 0 {
                            continue;
                        }
                        let bw = b.w.max(30) as f32;
                        let bh = b.h.max(18) as f32;
                        let top_left = rect.min
                            + egui::vec2((origin_x + b.x as f32) * fit_scale, (origin_y + b.y as f32) * fit_scale);
                        let r = egui::Rect::from_min_size(top_left, egui::vec2(bw * fit_scale, bh * fit_scale));
                        if !r.contains(pos) {
                            continue;
                        }
                        let is_tu = b.reg_n_mb == 1 || b.reg_tip == 1;
                        let is_bool = b.reg_tip == 0 && b.reg_bits.is_some();
                        let is_word16 = !is_tu && !is_bool && !matches!(b.reg_tip, 2 | 4 | 5);
                        let component_kind = preview_component_kind(b);
                        if component_kind == "bar" || component_kind == "gauge" || component_kind == "setpoint" {
                            let mut parts: Vec<String> = vec![format!("max={:.1}", preview_scale_max(b))];
                            if let Some(rules) = state.alarm_rules_by_reg.get(&b.reg_id) {
                                for rr in rules {
                                    if let Some(v) = rr.set_lo {
                                        parts.push(format!("LO<={v}"));
                                    }
                                    if let Some(v) = rr.set_lo_1 {
                                        parts.push(format!("LO_1<={v}"));
                                    }
                                    if let Some(v) = rr.set_hi_1 {
                                        parts.push(format!("HI_1>={v}"));
                                    }
                                    if let Some(v) = rr.set_hi {
                                        parts.push(format!("HI>={v}"));
                                    }
                                }
                            }
                            state.status = Some(format!("reg {} {}: {}", b.reg_id, component_kind, parts.join(", ")));
                        } else if component_kind == "trend" {
                            let samples = state.trend_history.get(&b.reg_id).map(|v| v.len()).unwrap_or(0);
                            state.status = Some(format!(
                                "reg {} trend: samples={}, current={}",
                                b.reg_id,
                                samples,
                                fmt_binding_live(b, *state.live_values.get(&b.reg_id).unwrap_or(&None), false)
                            ));
                        } else if component_kind == "button" {
                            state.status = Some(format!(
                                "reg {} button: mode=write-1.0, writable={}",
                                b.reg_id, b.writable
                            ));
                        } else if is_word16 {
                            state.bits16_reg_id = Some(b.reg_id);
                            state.bits16_open = true;
                            let live = state
                                .live_values
                                .get(&b.reg_id)
                                .copied()
                                .flatten()
                                .unwrap_or(0.0);
                            state.bits16_value = live.round().clamp(0.0, 65535.0) as u16;
                            state.status = Some(format!("reg {}: open 16-bit editor", b.reg_id));
                        } else if let Some(rules) = state.alarm_rules_by_reg.get(&b.reg_id) {
                            let mut parts: Vec<String> = Vec::new();
                            for rr in rules {
                                if let Some(v) = rr.set_lo {
                                    parts.push(format!("LO<={v}"));
                                }
                                if let Some(v) = rr.set_lo_1 {
                                    parts.push(format!("LO_1<={v}"));
                                }
                                if let Some(v) = rr.set_hi_1 {
                                    parts.push(format!("HI_1>={v}"));
                                }
                                if let Some(v) = rr.set_hi {
                                    parts.push(format!("HI>={v}"));
                                }
                            }
                            let levels = if parts.is_empty() {
                                "no thresholds".to_string()
                            } else {
                                parts.join(", ")
                            };
                            state.status = Some(format!("reg {} levels: {}", b.reg_id, levels));
                        } else {
                            state.status = Some(format!("reg {}: alarm levels not configured", b.reg_id));
                        }
                        break;
                    }
                }
    }

    actions
}

fn show_preview_window(ctx: &egui::Context, state: &mut UiLinkEditorState) -> Vec<UiLinkEditorAction> {
    let mut actions = Vec::new();
    if !state.window_template_open || !state.preview_open {
        return actions;
    }
    let mut preview_open = state.preview_open;
    egui::Window::new("KPZ Preview")
        .open(&mut preview_open)
        .resizable(true)
        .default_size([660.0, 560.0])
        .default_pos([1280.0, 620.0])
        .show(ctx, |ui| {
            actions.extend(show_preview_contents(ui, state));
        });
    state.preview_open = preview_open;
    actions
}

fn show_kp_viewer_window(
    ctx: &egui::Context,
    state: &mut UiLinkEditorState,
    selected_kpz: Option<i32>,
) -> Vec<UiLinkEditorAction> {
    let mut actions = Vec::new();
    if !state.kp_viewer_open {
        return actions;
    }
    let mut kp_viewer_open = state.kp_viewer_open;
    egui::Window::new("Просмотр окон КП")
        .open(&mut kp_viewer_open)
        .resizable(true)
        .default_size([760.0, 640.0])
        .default_pos([980.0, 100.0])
        .show(ctx, |ui| {
            if state.windows.is_empty() {
                if selected_kpz.is_none() {
                    ui.colored_label(
                        egui::Color32::from_rgb(240, 180, 60),
                        "Сначала выберите КПЗ в верхней панели приложения, затем нажмите «Reload windows».",
                    );
                } else {
                    ui.label("Список окон пуст. Нажмите «Reload windows» для загрузки.");
                }
            }
            ui.horizontal(|ui| {
                ui.label("Окно КП:");
                let mut selected = state.selected_window_id;
                let selected_text = selected
                    .and_then(|id| state.windows.iter().find(|w| w.id == id))
                    .map(|w| format!("{} [{}]", w.title, w.code))
                    .unwrap_or_else(|| "<окно не выбрано>".to_string());
                egui::ComboBox::from_id_salt("kp_viewer_window_select")
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        for w in &state.windows {
                            ui.selectable_value(&mut selected, Some(w.id), format!("{} [{}]", w.title, w.code));
                        }
                    });
                if selected != state.selected_window_id {
                    actions.push(UiLinkEditorAction::SelectWindow(selected));
                }
                if ui.button("Reload windows").clicked() {
                    actions.push(UiLinkEditorAction::ReloadWindows);
                }
                ui.checkbox(&mut state.trace_open, "TX/RX");
                ui.label(format!("timeout: {} ms", state.io_timeout_ms));
            });
            if state.selected_window_id.is_none() {
                ui.small("Выберите окно в списке выше, затем нажмите «Сосчитать» для опроса регистров.");
            } else {
                actions.extend(show_preview_contents(ui, state));
            }
            if let Some(err) = &state.err {
                ui.colored_label(egui::Color32::RED, err);
            } else if let Some(msg) = &state.status {
                ui.colored_label(egui::Color32::GREEN, msg);
            }
        });
    state.kp_viewer_open = kp_viewer_open;
    actions
}

fn show_trace_window(ctx: &egui::Context, state: &mut UiLinkEditorState) {
    if !(state.window_template_open || state.kp_viewer_open) || !state.trace_open {
        return;
    }
    egui::Window::new("TX/RX + Разбор")
        .open(&mut state.trace_open)
        .resizable(true)
        .default_size([980.0, 280.0])
        .default_pos([1210.0, 300.0])
        .show(ctx, |ui| {
            ui.label(format!("Время ожидания: {} мс", state.io_timeout_ms));
            if state.poll_trace.is_empty() {
                ui.label("Нажмите \"Сосчитать\" для вывода TX/RX и разбора.");
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.monospace(&state.poll_trace);
            });
        });
}

fn show_bits16_window(ctx: &egui::Context, state: &mut UiLinkEditorState) -> Vec<UiLinkEditorAction> {
    let mut actions = Vec::new();
    if !state.window_template_open || !state.bits16_open {
        return actions;
    }
    let Some(reg_id) = state.bits16_reg_id else {
        state.bits16_open = false;
        return actions;
    };
    let Some(binding) = state.bindings.iter().find(|b| b.reg_id == reg_id).cloned() else {
        state.bits16_open = false;
        return actions;
    };

    let mut open = state.bits16_open;
    egui::Window::new("Holding bits (16)")
        .open(&mut open)
        .resizable(true)
        .default_size([540.0, 300.0])
        .default_pos([980.0, 120.0])
        .show(ctx, |ui| {
            ui.label(format!(
                "reg={} name={} mb={} tip={}",
                binding.reg_id, binding.reg_name, binding.reg_mb, binding.reg_tip
            ));
            ui.label(format!(
                "live={} writable={}",
                fmt_binding_live(
                    &binding,
                    *state.live_values.get(&binding.reg_id).unwrap_or(&None),
                    !matches!(binding.reg_tip, 2 | 4 | 5) && !(binding.reg_tip == 0 && binding.reg_bits.is_some()) && !(binding.reg_n_mb == 1 || binding.reg_tip == 1)
                ),
                if binding.writable { "yes" } else { "no" }
            ));
            ui.separator();

            ui.horizontal(|ui| {
                let mut dec = state.bits16_value as i32;
                ui.label("DEC:");
                if ui
                    .add(egui::DragValue::new(&mut dec).range(0..=65535).speed(1))
                    .changed()
                {
                    state.bits16_value = dec as u16;
                }
                ui.label(format!("HEX: 0x{:04X}", state.bits16_value));
            });
            ui.horizontal(|ui| {
                if ui.button("Set all").clicked() {
                    state.bits16_value = 0xFFFF;
                }
                if ui.button("Clear all").clicked() {
                    state.bits16_value = 0;
                }
                if ui.button("Invert").clicked() {
                    state.bits16_value = !state.bits16_value;
                }
                if ui.button("Load live").clicked() {
                    let live = state
                        .live_values
                        .get(&binding.reg_id)
                        .copied()
                        .flatten()
                        .unwrap_or(0.0);
                    state.bits16_value = live.round().clamp(0.0, 65535.0) as u16;
                }
            });

            ui.separator();
            egui::Grid::new("bits16_grid").num_columns(8).show(ui, |ui| {
                for bit in (0..16).rev() {
                    let mut on = ((state.bits16_value >> bit) & 1) != 0;
                    if ui.checkbox(&mut on, format!("b{}", bit)).changed() {
                        if on {
                            state.bits16_value |= 1u16 << bit;
                        } else {
                            state.bits16_value &= !(1u16 << bit);
                        }
                    }
                    if bit % 8 == 0 {
                        ui.end_row();
                    }
                }
            });

            ui.separator();
            if ui
                .add_enabled(binding.writable, egui::Button::new("Write word"))
                .clicked()
            {
                actions.push(UiLinkEditorAction::WriteValue {
                    reg_id: binding.reg_id,
                    val: state.bits16_value as f64,
                });
            }
            if !binding.writable {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "Readonly: enable wr in Bindings to allow write.",
                );
            }
        });
    state.bits16_open = open;
    actions
}

pub fn show_ui_link_editor(
    ctx: &egui::Context,
    state: &mut UiLinkEditorState,
    selected_kpz: Option<i32>,
    selected_kpz_name: &str,
    groups: &[GroupRow],
) -> Vec<UiLinkEditorAction> {
    let mut actions = Vec::new();
    state.open = state.window_template_open || state.kp_template_open || state.kp_binding_open || state.kp_viewer_open;
    if !state.open {
        return actions;
    }

    if state.kp_template_open {
        let mut kp_template_open = state.kp_template_open;
        egui::Window::new("Шаблон набора окон")
            .open(&mut kp_template_open)
            .resizable(true)
            .default_size([900.0, 520.0])
            .default_pos([40.0, 100.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Reload KP templates").clicked() {
                        actions.push(UiLinkEditorAction::ReloadKpTemplates);
                    }
                    if ui.button("Reload templates").clicked() {
                        actions.push(UiLinkEditorAction::ReloadTemplates);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("KP template:");
                    let mut sel_kp_tpl = state.selected_kp_template_id;
                    let kp_tpl_text = sel_kp_tpl
                        .and_then(|id| state.kp_templates.iter().find(|t| t.id == id))
                        .map(|t| format!("{} [{}]", t.title, t.code))
                        .unwrap_or_else(|| "<новый kp template>".to_string());
                    egui::ComboBox::from_id_salt("kp_template_window_select")
                        .selected_text(kp_tpl_text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut sel_kp_tpl, None, "<новый kp template>");
                            for t in &state.kp_templates {
                                ui.selectable_value(&mut sel_kp_tpl, Some(t.id), format!("{} [{}]", t.title, t.code));
                            }
                        });
                    if sel_kp_tpl != state.selected_kp_template_id {
                        state.selected_kp_template_id = sel_kp_tpl;
                        actions.push(UiLinkEditorAction::SelectKpTemplate(sel_kp_tpl));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Code:");
                    ui.add(egui::TextEdit::singleline(&mut state.kp_template_code).desired_width(120.0));
                    ui.label("Title:");
                    ui.add(egui::TextEdit::singleline(&mut state.kp_template_title).desired_width(220.0));
                    if ui.button("Save KP template").clicked() {
                        actions.push(UiLinkEditorAction::SaveKpTemplate);
                    }
                    if ui
                        .add_enabled(state.selected_kp_template_id.is_some(), egui::Button::new("Delete KP template"))
                        .clicked()
                    {
                        actions.push(UiLinkEditorAction::DeleteKpTemplate);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Description:");
                    ui.add(egui::TextEdit::singleline(&mut state.kp_template_description).desired_width(520.0));
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Window template:");
                    let mut sel_tpl = state.selected_template_id;
                    let tpl_text = sel_tpl
                        .and_then(|id| state.templates.iter().find(|t| t.id == id))
                        .map(|t| format!("{} [{}]", t.title, t.code))
                        .unwrap_or_else(|| "<none>".to_string());
                    egui::ComboBox::from_id_salt("kp_template_window_template_select")
                        .selected_text(tpl_text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut sel_tpl, None, "<none>");
                            for t in &state.templates {
                                ui.selectable_value(&mut sel_tpl, Some(t.id), format!("{} [{}]", t.title, t.code));
                            }
                        });
                    if sel_tpl != state.selected_template_id {
                        state.selected_template_id = sel_tpl;
                        actions.push(UiLinkEditorAction::SelectTemplate(sel_tpl));
                    }
                    if ui.button("Add window template").clicked() {
                        actions.push(UiLinkEditorAction::AddWindowTemplateToKpTemplate);
                    }
                    if ui.button("Reload rows").clicked() {
                        actions.push(UiLinkEditorAction::ReloadKpTemplateWindows);
                    }
                });
                if state.templates.is_empty() {
                    ui.small("Список шаблонов окон пуст. Нажмите Reload templates или создайте шаблон окна.");
                }
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for row in state.kp_template_windows.clone() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{} [{}] sort={}", row.window_template_title, row.window_template_code, row.sort_order));
                            if ui.small_button("Remove").clicked() {
                                actions.push(UiLinkEditorAction::RemoveWindowTemplateFromKpTemplate { window_template_id: row.window_template_id });
                            }
                        });
                    }
                });
                if let Some(err) = &state.err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &state.status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }
            });
        state.kp_template_open = kp_template_open;
    }

    if state.kp_binding_open {
        let mut kp_binding_open = state.kp_binding_open;
        egui::Window::new("Привязка КП к шаблону набора окон")
            .open(&mut kp_binding_open)
            .resizable(true)
            .default_size([760.0, 320.0])
            .default_pos([80.0, 120.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "KPZ: {} {}",
                        selected_kpz.map(|v| v.to_string()).unwrap_or_else(|| "<none>".to_string()),
                        selected_kpz_name
                    ));
                    if ui.button("Reload KP templates").clicked() {
                        actions.push(UiLinkEditorAction::ReloadKpTemplates);
                    }
                    if ui.button("Reload KP link").clicked() {
                        actions.push(UiLinkEditorAction::ReloadKpzKpTemplateLink);
                    }
                    if ui.small_button("?").clicked() {
                        state.kp_binding_help_open = !state.kp_binding_help_open;
                    }
                });
                if state.kp_binding_help_open {
                    ui.group(|ui| {
                        ui.label("Как работать:");
                        ui.label("1) Выберите КП в верхней строке главного окна.");
                        ui.label("2) Здесь выберите шаблон набора окон.");
                        ui.label("3) Нажмите `Apply KP template to KPZ`, чтобы привязать набор и создать окна для этого КП.");
                        ui.label("4) Повторное нажатие не создает второй комплект, если окна уже есть.");
                        ui.label("5) Блок `Окна выбранного шаблона` показывает, что входит в набор.");
                        ui.label("6) Блок `Окна выбранного КП` показывает, что уже создано у текущего КП.");
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Шаблон набора окон:");
                    let mut sel_kp_tpl = state.selected_kp_binding_template_id;
                    let kp_tpl_text = sel_kp_tpl
                        .and_then(|id| state.kp_templates.iter().find(|t| t.id == id))
                        .map(|t| format!("{} [{}]", t.title, t.code))
                        .unwrap_or_else(|| "<none>".to_string());
                    egui::ComboBox::from_id_salt("kp_binding_editor_select")
                        .selected_text(kp_tpl_text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut sel_kp_tpl, None, "<none>");
                            for t in &state.kp_templates {
                                ui.selectable_value(&mut sel_kp_tpl, Some(t.id), format!("{} [{}]", t.title, t.code));
                            }
                        });
                    if sel_kp_tpl != state.selected_kp_binding_template_id {
                        actions.push(UiLinkEditorAction::SelectKpBindingTemplate(sel_kp_tpl));
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(state.selected_kp_binding_template_id.is_some(), egui::Button::new("Apply KP template to KPZ"))
                        .clicked()
                    {
                        actions.push(UiLinkEditorAction::ApplyKpTemplateToKpz);
                    }
                    if ui.button("Unlink + delete created windows").clicked() {
                        actions.push(UiLinkEditorAction::UnlinkKpTemplateAndDeleteWindows);
                    }
                });
                let link_text = state
                    .kpz_kp_template_link
                    .as_ref()
                    .map(|l| format!("Текущая привязка: {} [{}]", l.kp_template_title, l.kp_template_code))
                    .unwrap_or_else(|| "Текущая привязка: <none>".to_string());
                ui.label(link_text);
                ui.small("Порядок: выберите шаблон набора и нажмите Apply KP template to KPZ. Повторный комплект окон не создается.");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Окна выбранного шаблона:");
                    ui.label(format!("count: {}", state.kp_binding_template_windows.len()));
                });
                if state.kp_binding_template_windows.is_empty() {
                    ui.small("Выберите шаблон набора окон, чтобы увидеть его состав.");
                } else {
                    egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
                        for row in &state.kp_binding_template_windows {
                            ui.label(format!(
                                "{} [{}] sort={}",
                                row.window_template_title, row.window_template_code, row.sort_order
                            ));
                        }
                    });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Окна выбранного КП:");
                    ui.label(format!("count: {}", state.windows.len()));
                });
                if state.windows.is_empty() {
                    ui.small("Для этого КП окна еще не созданы или список не обновлен.");
                } else {
                    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                        for w in &state.windows {
                            let desc = w.description.as_deref().map(str::trim).unwrap_or("");
                            if desc.is_empty() {
                                ui.label(format!("{} [{}]", w.title, w.code));
                            } else {
                                ui.label(format!("{} [{}] - {}", w.title, w.code, desc));
                            }
                        }
                    });
                }
                if let Some(err) = &state.err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &state.status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }
            });
        state.kp_binding_open = kp_binding_open;
    }

    if !state.window_template_open {
        actions.extend(show_kp_viewer_window(ctx, state, selected_kpz));
        show_trace_window(ctx, state);
        state.open = state.window_template_open || state.kp_template_open || state.kp_binding_open || state.kp_viewer_open;
        return actions;
    }

    state.template_editor_mode = true;
    state.kp_template_editor_mode = false;
    state.kp_binding_editor_mode = false;

    let mut open = state.window_template_open;
    egui::Window::new("Шаблон окна")
        .open(&mut open)
        .movable(true)
        .collapsible(true)
        .resizable(true)
        .default_pos([8.0, 88.0])
        .default_size([1240.0, 760.0])
        .show(ctx, |ui| {
            ui.separator();

            ui.horizontal(|ui| {
                if !state.template_editor_mode {
                    ui.label(format!(
                        "KPZ: {} {}",
                        selected_kpz.map(|v| v.to_string()).unwrap_or_else(|| "<none>".to_string()),
                        selected_kpz_name
                    ));
                    ui.separator();
                }
                ui.small(if state.kp_binding_editor_mode {
                    "Выберите шаблон набора окон, привяжите его к текущему КПЗ и создайте окна."
                } else if state.kp_template_editor_mode {
                    "Соберите набор из шаблонов окон и привяжите его к КПЗ."
                } else if state.template_editor_mode {
                    "Редактирование шаблона окна: выбор читает шаблон, New очищает форму, Save сохраняет."
                } else {
                    "Рабочий режим."
                });
            });

            if state.kp_binding_editor_mode {
                ui.horizontal(|ui| {
                    if ui.button("Reload KP templates").clicked() {
                        actions.push(UiLinkEditorAction::ReloadKpTemplates);
                    }
                    if ui.button("Reload KP link").clicked() {
                        actions.push(UiLinkEditorAction::ReloadKpzKpTemplateLink);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Шаблон набора окон:");
                    let mut sel_kp_tpl = state.selected_kp_binding_template_id;
                    let kp_tpl_text = sel_kp_tpl
                        .and_then(|id| state.kp_templates.iter().find(|t| t.id == id))
                        .map(|t| format!("{} [{}]", t.title, t.code))
                        .unwrap_or_else(|| "<none>".to_string());
                    egui::ComboBox::from_id_salt("ui_link_kp_binding_select")
                        .selected_text(kp_tpl_text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut sel_kp_tpl, None, "<none>");
                            for t in &state.kp_templates {
                                ui.selectable_value(
                                    &mut sel_kp_tpl,
                                    Some(t.id),
                                    format!("{} [{}]", t.title, t.code),
                                );
                            }
                        });
                    if sel_kp_tpl != state.selected_kp_binding_template_id {
                        state.selected_kp_binding_template_id = sel_kp_tpl;
                        actions.push(UiLinkEditorAction::SelectKpBindingTemplate(sel_kp_tpl));
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(state.selected_kp_binding_template_id.is_some(), egui::Button::new("Apply KP template to KPZ"))
                        .clicked()
                    {
                        actions.push(UiLinkEditorAction::ApplyKpTemplateToKpz);
                    }
                    if ui.button("Unlink + delete created windows").clicked() {
                        actions.push(UiLinkEditorAction::UnlinkKpTemplateAndDeleteWindows);
                    }
                });
                ui.separator();
                let link_text = state
                    .kpz_kp_template_link
                    .as_ref()
                    .map(|l| format!("Текущая привязка: {} [{}]", l.kp_template_title, l.kp_template_code))
                    .unwrap_or_else(|| "Текущая привязка: <none>".to_string());
                ui.label(link_text);
                if let Some(err) = &state.err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &state.status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }
            } else {
            ui.horizontal(|ui| {
                if !state.template_editor_mode && !state.kp_template_editor_mode {
                    ui.label("Окно KPZ:");
                    let mut selected = state.selected_window_id;
                    let selected_text = selected
                        .and_then(|id| state.windows.iter().find(|w| w.id == id))
                        .map(|w| {
                            let desc = w.description.as_deref().map(str::trim).unwrap_or("");
                            if desc.is_empty() {
                                format!("{} [{}]", w.title, w.code)
                            } else {
                                format!("{} [{}] - {}", w.title, w.code, desc)
                            }
                        })
                        .unwrap_or_else(|| "<окно не выбрано>".to_string());
                    egui::ComboBox::from_id_salt("ui_link_window_select")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            for w in &state.windows {
                                let desc = w.description.as_deref().map(str::trim).unwrap_or("");
                                let label = if desc.is_empty() {
                                    format!("{} [{}]", w.title, w.code)
                                } else {
                                    format!("{} [{}] - {}", w.title, w.code, desc)
                                };
                                ui.selectable_value(
                                    &mut selected,
                                    Some(w.id),
                                    label,
                                );
                            }
                        });
                    if selected != state.selected_window_id {
                        state.selected_window_id = selected;
                        actions.push(UiLinkEditorAction::SelectWindow(selected));
                    }
                } else if state.template_editor_mode {
                    ui.label("Шаблон:");
                    let mut sel_tpl = state.selected_template_id;
                    let tpl_text = sel_tpl
                        .and_then(|id| state.templates.iter().find(|t| t.id == id))
                        .map(|t| format!("{} [{}]", t.title, t.code))
                        .unwrap_or_else(|| "<новый шаблон (не выбран)>".to_string());
                    egui::ComboBox::from_id_salt("ui_link_template_select_editor")
                        .selected_text(tpl_text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut sel_tpl, None, "<новый шаблон>");
                            for t in &state.templates {
                                ui.selectable_value(
                                    &mut sel_tpl,
                                    Some(t.id),
                                    format!("{} [{}]", t.title, t.code),
                                );
                            }
                        });
                    if sel_tpl != state.selected_template_id {
                        state.selected_template_id = sel_tpl;
                        actions.push(UiLinkEditorAction::SelectTemplate(sel_tpl));
                    }
                } else {
                    ui.label("KP template:");
                    let mut sel_kp_tpl = state.selected_kp_template_id;
                    let kp_tpl_text = sel_kp_tpl
                        .and_then(|id| state.kp_templates.iter().find(|t| t.id == id))
                        .map(|t| format!("{} [{}]", t.title, t.code))
                        .unwrap_or_else(|| "<новый kp template>".to_string());
                    egui::ComboBox::from_id_salt("ui_link_kp_template_select_editor")
                        .selected_text(kp_tpl_text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut sel_kp_tpl, None, "<новый kp template>");
                            for t in &state.kp_templates {
                                ui.selectable_value(
                                    &mut sel_kp_tpl,
                                    Some(t.id),
                                    format!("{} [{}]", t.title, t.code),
                                );
                            }
                        });
                    if sel_kp_tpl != state.selected_kp_template_id {
                        state.selected_kp_template_id = sel_kp_tpl;
                        actions.push(UiLinkEditorAction::SelectKpTemplate(sel_kp_tpl));
                    }
                }

                ui.label("Code:");
                if state.kp_template_editor_mode {
                    ui.add(egui::TextEdit::singleline(&mut state.kp_template_code).desired_width(120.0));
                } else {
                    ui.add(egui::TextEdit::singleline(&mut state.window_code).desired_width(120.0));
                }
                ui.label("Title:");
                if state.kp_template_editor_mode {
                    ui.add(egui::TextEdit::singleline(&mut state.kp_template_title).desired_width(220.0));
                } else {
                    ui.add(egui::TextEdit::singleline(&mut state.window_title).desired_width(220.0));
                }
                if ui
                    .add_enabled(
                        if state.kp_template_editor_mode {
                            true
                        } else if state.template_editor_mode {
                            true
                        } else {
                            state.selected_window_id.is_some()
                        },
                        egui::Button::new(if state.kp_template_editor_mode {
                            "Save KP template"
                        } else if state.template_editor_mode {
                            "Save template"
                        } else {
                            "Save window"
                        }),
                    )
                    .clicked()
                {
                    if state.kp_template_editor_mode {
                        actions.push(UiLinkEditorAction::SaveKpTemplate);
                    } else {
                        actions.push(UiLinkEditorAction::UpsertWindow);
                    }
                }
                if !state.template_editor_mode
                    && ui
                        .add_enabled(
                            if state.kp_template_editor_mode {
                                state.selected_kp_template_id.is_some()
                            } else {
                                state.selected_window_id.is_some()
                            },
                            egui::Button::new(if state.kp_template_editor_mode {
                                "Delete KP template"
                            } else {
                                "Delete window"
                            }),
                        )
                        .clicked()
                {
                    if state.kp_template_editor_mode {
                        actions.push(UiLinkEditorAction::DeleteKpTemplate);
                    } else {
                        actions.push(UiLinkEditorAction::DeleteWindow);
                    }
                }
                if ui.button("Focus Drag Window").clicked() {
                    state.layout_open = true;
                    state.preview_open = false;
                    state.layout_focus_request = true;
                }
                if !state.template_editor_mode && ui.button("Reload windows").clicked() {
                    actions.push(UiLinkEditorAction::ReloadWindows);
                }
                if ui
                    .button(if state.kp_template_editor_mode {
                        "New KP template"
                    } else if state.template_editor_mode {
                        "New template"
                    } else {
                        "Clear draft"
                    })
                    .clicked()
                {
                    state.clear_for_new_window();
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Reload templates").clicked() {
                    actions.push(UiLinkEditorAction::ReloadTemplates);
                }
                if state.template_editor_mode
                    && ui
                        .add_enabled(
                            state.selected_template_id.is_some(),
                            egui::Button::new("Sync images to windows"),
                        )
                        .on_hover_text("Copy missing image background items from this template to active windows with the same code")
                        .clicked()
                {
                    actions.push(UiLinkEditorAction::SyncTemplateImagesToWindows);
                }
                if state.template_editor_mode
                    && ui
                        .add_enabled(
                            state.selected_template_id.is_some(),
                            egui::Button::new("Update window images"),
                        )
                        .on_hover_text("Overwrite existing image background geometry/path/fit in matching active windows from this template")
                        .clicked()
                {
                    actions.push(UiLinkEditorAction::UpdateTemplateImagesInWindows);
                }
                if !state.template_editor_mode && ui.button("Reload KP templates").clicked() {
                    actions.push(UiLinkEditorAction::ReloadKpTemplates);
                }
                if !state.template_editor_mode {
                    ui.label("Template:");
                    let mut sel_tpl = state.selected_template_id;
                    let tpl_text = sel_tpl
                        .and_then(|id| state.templates.iter().find(|t| t.id == id))
                        .map(|t| format!("{} [{}]", t.title, t.code))
                        .unwrap_or_else(|| "<none>".to_string());
                    egui::ComboBox::from_id_salt("ui_link_template_select")
                        .selected_text(tpl_text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut sel_tpl, None, "<none>");
                            for t in &state.templates {
                                ui.selectable_value(
                                    &mut sel_tpl,
                                    Some(t.id),
                                    format!("{} [{}]", t.title, t.code),
                                );
                            }
                        });
                    if sel_tpl != state.selected_template_id {
                        state.selected_template_id = sel_tpl;
                        actions.push(UiLinkEditorAction::SelectTemplate(sel_tpl));
                    }
                }
                if !state.template_editor_mode
                    && ui
                        .add_enabled(
                            !state.kp_template_editor_mode && state.selected_template_id.is_some(),
                            egui::Button::new("Create window from template"),
                        )
                        .clicked()
                {
                    actions.push(UiLinkEditorAction::CreateWindowFromTemplate);
                }
                if !state.template_editor_mode
                    && ui
                        .add_enabled(state.selected_template_id.is_some(), egui::Button::new("Link to KPZ"))
                        .clicked()
                {
                    actions.push(UiLinkEditorAction::LinkTemplateToKpz);
                }
                if !state.template_editor_mode
                    && ui
                        .add_enabled(state.selected_template_id.is_some(), egui::Button::new("Unlink from KPZ"))
                        .clicked()
                {
                    actions.push(UiLinkEditorAction::UnlinkTemplateFromKpz);
                }
                if !state.template_editor_mode && ui.button("Reload links").clicked() {
                    actions.push(UiLinkEditorAction::ReloadTemplateLinks);
                }
                if state.kp_template_editor_mode {
                    ui.separator();
                    ui.label("KP template:");
                    let mut sel_kp_tpl = state.selected_kp_template_id;
                    let kp_tpl_text = sel_kp_tpl
                        .and_then(|id| state.kp_templates.iter().find(|t| t.id == id))
                        .map(|t| format!("{} [{}]", t.title, t.code))
                        .unwrap_or_else(|| "<none>".to_string());
                    egui::ComboBox::from_id_salt("ui_link_kp_template_select_top")
                        .selected_text(kp_tpl_text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut sel_kp_tpl, None, "<none>");
                            for t in &state.kp_templates {
                                ui.selectable_value(
                                    &mut sel_kp_tpl,
                                    Some(t.id),
                                    format!("{} [{}]", t.title, t.code),
                                );
                            }
                        });
                    if sel_kp_tpl != state.selected_kp_template_id {
                        state.selected_kp_template_id = sel_kp_tpl;
                        actions.push(UiLinkEditorAction::SelectKpTemplate(sel_kp_tpl));
                    }
                    if ui
                        .add_enabled(state.selected_template_id.is_some() && state.selected_kp_template_id.is_some(), egui::Button::new("Add window template"))
                        .clicked()
                    {
                        actions.push(UiLinkEditorAction::AddWindowTemplateToKpTemplate);
                    }
                    if ui.button("Reload KP rows").clicked() {
                        actions.push(UiLinkEditorAction::ReloadKpTemplateWindows);
                    }
                    if ui.button("Reload KP link").clicked() {
                        actions.push(UiLinkEditorAction::ReloadKpzKpTemplateLink);
                    }
                }
                ui.separator();
                ui.label(format!(
                    "KPZ: {} {}",
                    selected_kpz.map(|v| v.to_string()).unwrap_or_else(|| "<none>".to_string()),
                    selected_kpz_name
                ));
                if state.kp_template_editor_mode {
                    let caption = state
                        .kpz_kp_template_link
                        .as_ref()
                        .map(|l| format!("linked KP template: {} [{}]", l.kp_template_title, l.kp_template_code))
                        .unwrap_or_else(|| "linked KP template: <none>".to_string());
                    ui.small(caption);
                } else {
                    ui.small("Create window from template использует текущие Code/Title/Description как метаданные нового окна.");
                }
                if state.template_editor_mode {
                    ui.separator();
                    ui.small("Reload templates перечитывает список шаблонов из базы.");
                }
                if ui.small_button("?").clicked() {
                    state.help_open = true;
                }
                ui.checkbox(&mut state.web_safe_preview, "Web-safe");
                ui.checkbox(&mut state.preview_open, "Preview");
                ui.checkbox(&mut state.layout_open, "Layout");
                ui.checkbox(&mut state.trace_open, "TX/RX");
            });
            if !state.template_editor_mode && !state.kp_template_editor_mode {
                ui.horizontal(|ui| {
                    ui.label("Code: технический уникальный код окна. Title: отображаемое имя.");
                    ui.separator();
                    ui.small("Новое окно создается только через Create window from template.");
                });
            } else if state.template_editor_mode {
                ui.horizontal(|ui| {
                    ui.label("Редактирование шаблона окна: выбор шаблона загружает его из БД. Save template — метаданные (Code/Title/Description). Save all — привязки и layout. New template — очистить форму.");
                });
            } else {
                ui.horizontal(|ui| {
                    ui.label("KP template: набор window templates для типового КП.");
                    ui.separator();
                    ui.small("Save KP template сохраняет метаданные набора. Add window template добавляет выбранный window template в состав.");
                });
            }
            ui.horizontal(|ui| {
                ui.label("Description:");
                let changed = if state.template_editor_mode {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.window_description)
                            .desired_width(620.0)
                            .desired_rows(2),
                    ).changed()
                } else if state.kp_template_editor_mode {
                    ui.add(egui::TextEdit::singleline(&mut state.kp_template_description).desired_width(620.0)).changed()
                } else {
                    ui.add(egui::TextEdit::singleline(&mut state.window_description).desired_width(620.0)).changed()
                };
                if changed {
                    state.dirty = true;
                }
                ui.separator();
                if let Some(err) = &state.err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &state.status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }
            });

            if state.kp_template_editor_mode {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.heading("KP template windows");
                    ui.label(format!("count: {}", state.kp_template_windows.len()));
                });
                if state.kp_template_windows.is_empty() {
                    ui.label("Состав пуст. Выберите window template сверху и нажмите Add window template.");
                } else {
                    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                        for row in state.kp_template_windows.clone() {
                            ui.horizontal(|ui| {
                                ui.label(format!(
                                    "{} [{}] sort={}",
                                    row.window_template_title, row.window_template_code, row.sort_order
                                ));
                                if row.is_default {
                                    ui.label("default");
                                }
                                if ui.small_button("Remove").clicked() {
                                    actions.push(UiLinkEditorAction::RemoveWindowTemplateFromKpTemplate {
                                        window_template_id: row.window_template_id,
                                    });
                                }
                            });
                        }
                    });
                }
            }
            }

            ui.separator();
            let total_w = ui.available_width().max(600.0);
            let w_left = (total_w * 0.10).max(120.0);
            let w_mid = (total_w * 0.10).max(160.0);
            let mut w_right = total_w - w_left - w_mid;
            if w_right < 420.0 {
                w_right = 420.0;
            }
            ui.horizontal(|ui| {
                let col_h = ui.available_height().max(280.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(w_left, col_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let mut groups_changed = false;
                        ui.heading("Groups");
                        ui.label("Selected groups:");
                        let caption = state.selected_groups_caption(groups);
                        egui::ComboBox::from_id_salt("ui_link_groups_select")
                            .selected_text(caption)
                            .show_ui(ui, |ui| {
                                for g in groups {
                                    let mut checked = state.groups_selected.contains(&g.id);
                                    let label = if g.name.is_empty() {
                                        g.id.to_string()
                                    } else {
                                        format!("{} - {}", g.id, g.name)
                                    };
                                    if ui.checkbox(&mut checked, label).changed() {
                                        if checked {
                                            state.groups_selected.insert(g.id);
                                        } else {
                                            state.groups_selected.remove(&g.id);
                                        }
                                        state.dirty = true;
                                        groups_changed = true;
                                    }
                                }
                            });
                        ui.label(format!("count: {}", state.groups_selected.len()));
                        if ui.button("Reload regs by groups").clicked() || groups_changed {
                            actions.push(UiLinkEditorAction::ReloadRegs);
                        }
                    },
                );

                ui.separator();

                ui.allocate_ui_with_layout(
                    egui::vec2(w_mid, col_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.heading("Available regs");
                        ui.horizontal(|ui| {
                            ui.label("find:");
                            ui.add(egui::TextEdit::singleline(&mut state.regs_filter).desired_width(140.0));
                        });
                        if !state.groups_selected.is_empty() {
                            if let Some(gf) = state.regs_group_filter
                                && !state.groups_selected.contains(&gf)
                            {
                                state.regs_group_filter = None;
                            }
                            ui.horizontal(|ui| {
                                ui.label("group:");
                                let mut gf = state.regs_group_filter;
                                let gf_text = gf
                                    .and_then(|id| groups.iter().find(|g| g.id == id))
                                    .map(|g| {
                                        if g.name.is_empty() {
                                            g.id.to_string()
                                        } else {
                                            format!("{} - {}", g.id, g.name)
                                        }
                                    })
                                    .unwrap_or_else(|| "<all selected>".to_string());
                                egui::ComboBox::from_id_salt("ui_link_regs_group_filter")
                                    .selected_text(gf_text)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut gf, None, "<all selected>");
                                        for gid in &state.groups_selected {
                                            if let Some(g) = groups.iter().find(|x| x.id == *gid) {
                                                let txt = if g.name.is_empty() {
                                                    g.id.to_string()
                                                } else {
                                                    format!("{} - {}", g.id, g.name)
                                                };
                                                ui.selectable_value(&mut gf, Some(g.id), txt);
                                            }
                                        }
                                    });
                                state.regs_group_filter = gf;
                            });
                            let f = state.regs_filter.trim().to_lowercase();
                            let mut pick = state.reg_pick_one;
                            let pick_text = pick
                                .and_then(|id| state.regs_available.iter().find(|r| r.id == id))
                                .map(|r| format!("{} | g={} | mb={} | {}", r.id, r.grup, r.mb, r.name))
                                .unwrap_or_else(|| "<select reg>".to_string());
                            egui::ComboBox::from_id_salt("ui_link_reg_pick_one")
                                .selected_text(pick_text)
                                .show_ui(ui, |ui| {
                                    for r in &state.regs_available {
                                        if let Some(gf) = state.regs_group_filter
                                            && r.grup != gf
                                        {
                                            continue;
                                        }
                                        let text = format!("{} | g={} | mb={} | {}", r.id, r.grup, r.mb, r.name);
                                        if !f.is_empty() && !text.to_lowercase().contains(&f) {
                                            continue;
                                        }
                                        ui.selectable_value(&mut pick, Some(r.id), text);
                                    }
                                });
                            state.reg_pick_one = pick;
                            ui.horizontal(|ui| {
                                if ui.button("Add one ->").clicked()
                                    && let Some(id) = state.reg_pick_one
                                {
                                    state.regs_selected.insert(id);
                                    state.add_selected_regs_to_bindings();
                                    state.regs_selected.clear();
                                }
                                if ui.button("Add all ->").clicked() {
                                    let mut ids = Vec::new();
                                    for r in &state.regs_available {
                                        if let Some(gf) = state.regs_group_filter
                                            && r.grup != gf
                                        {
                                            continue;
                                        }
                                        let text = format!("{} | g={} | mb={} | {}", r.id, r.grup, r.mb, r.name);
                                        if !f.is_empty() && !text.to_lowercase().contains(&f) {
                                            continue;
                                        }
                                        ids.push(r.id);
                                    }
                                    for id in ids {
                                        state.regs_selected.insert(id);
                                    }
                                    state.add_selected_regs_to_bindings();
                                    state.regs_selected.clear();
                                }
                            });
                        } else {
                            ui.label("Select group(s) first.");
                        }
                    },
                );

                ui.separator();

                ui.allocate_ui_with_layout(
                    egui::vec2(w_right, col_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.heading("Bindings");
                        ui.horizontal(|ui| {
                            ui.label(format!("selected: {}", state.selected_binding_reg_ids.len()));
                            if ui.button("Sel all").clicked() {
                                state.select_all_bindings();
                            }
                            if ui.button("Clear").clicked() {
                                state.clear_binding_selection();
                            }
                            let can_add_layout_items = !state.kp_template_editor_mode;
                            let add_text_btn = ui
                                .add_enabled(can_add_layout_items, egui::Button::new("Add text"))
                                .on_hover_text(if can_add_layout_items {
                                    "Add text item to the current window/template layout"
                                } else {
                                    "Text/image items are saved only in Window template editor, not KP template editor"
                                });
                            if add_text_btn.clicked() {
                                state.add_text_item();
                            }
                            let add_image_btn = ui
                                .add_enabled(can_add_layout_items, egui::Button::new("Add image"))
                                .on_hover_text(if can_add_layout_items {
                                    "Add image item to the current window/template layout"
                                } else {
                                    "Text/image items are saved only in Window template editor, not KP template editor"
                                });
                            if add_image_btn.clicked() {
                                state.add_image_item();
                                state.status = Some(
                                    "image added: keep path ui_images/scheme.png and press Save all in Window template editor"
                                        .to_string(),
                                );
                            }
                            let can_save = if state.kp_template_editor_mode {
                                false
                            } else if state.template_editor_mode {
                                state.selected_template_id.is_some()
                            } else {
                                state.selected_window_id.is_some()
                            };
                            let save_btn = ui.add_enabled(can_save, egui::Button::new("Save all"));
                            let save_btn = if state.template_editor_mode {
                                save_btn.on_hover_text("Сохранить привязки и layout выбранного шаблона в БД")
                            } else {
                                save_btn
                            };
                            if save_btn.clicked() {
                                actions.push(UiLinkEditorAction::SaveAll);
                            }
                            if state.dirty {
                                ui.label("Unsaved changes");
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("W:");
                            ui.add(egui::TextEdit::singleline(&mut state.batch_w_input).desired_width(44.0));
                            ui.label("H:");
                            ui.add(egui::TextEdit::singleline(&mut state.batch_h_input).desired_width(44.0));
                            ui.label("Gap:");
                            ui.add(egui::TextEdit::singleline(&mut state.batch_gap_input).desired_width(40.0));
                            if ui.button("Apply size").clicked() {
                                state.batch_set_size();
                            }
                            if ui.button("Stack V").clicked() {
                                state.batch_arrange(true);
                            }
                            if ui.button("Stack H").clicked() {
                                state.batch_arrange(false);
                            }
                            let has_selected_image = state
                                .bindings
                                .iter()
                                .any(|b| state.selected_binding_reg_ids.contains(&b.reg_id) && is_image_item(b));
                            if ui
                                .add_enabled(has_selected_image, egui::Button::new("Img fit"))
                                .on_hover_text("Fit selected image background to current controls/layout bounds")
                                .clicked()
                            {
                                state.fit_selected_images_to_layout();
                            }
                            if ui
                                .add_enabled(has_selected_image, egui::Button::new("Img back"))
                                .on_hover_text("Move selected image items before controls in layer/order")
                                .clicked()
                            {
                                state.send_selected_images_to_back();
                            }
                            if ui
                                .add_enabled(!state.preview_image_cache.is_empty(), egui::Button::new("Img cache clear"))
                                .on_hover_text("Clear desktop preview image texture cache")
                                .clicked()
                            {
                                state.preview_image_cache.clear();
                                state.status = Some("preview image cache cleared".to_string());
                            }
                        });
                        ui.separator();
                        if state.bindings.is_empty() {
                            ui.label("Нет привязок. Выберите регистр в Available regs и нажмите Add one ->");
                        }
                        egui::ScrollArea::vertical().show(ui, |ui| {
                    let rows = state.bindings.clone();
                    let mut component_kind_changes: Vec<(i32, Option<String>)> = vec![];
                    for b in rows {
                        ui.horizontal(|ui| {
                            let mut mult = state.selected_binding_reg_ids.contains(&b.reg_id);
                            if ui.checkbox(&mut mult, "").changed() {
                                if mult {
                                    state.selected_binding_reg_ids.insert(b.reg_id);
                                } else {
                                    state.selected_binding_reg_ids.remove(&b.reg_id);
                                }
                            }
                            let selected = state.selected_binding_reg_id == Some(b.reg_id);
                            let row_title = if is_image_item(&b) {
                                format!("{}: [IMAGE]", b.pos)
                            } else if b.is_text || b.reg_id <= 0 {
                                format!("{}: [TEXT]", b.pos)
                            } else {
                                format!("{}: {} (mb={})", b.pos, b.reg_id, b.reg_mb)
                            };
                            if ui.selectable_label(selected, row_title).clicked() {
                                state.selected_binding_reg_id = Some(b.reg_id);
                                state.selected_binding_reg_ids.insert(b.reg_id);
                            }
                            ui.label(&b.reg_name);
                            if let Some(cur) = state.bindings.iter().find(|x| x.reg_id == b.reg_id) {
                                ui.label(format!("xy=({}, {}) wh=({}, {})", cur.x, cur.y, cur.w, cur.h));
                            }
                            if let Some(v) = state.live_values.get(&b.reg_id) {
                                let is_tu = b.reg_n_mb == 1 || b.reg_tip == 1;
                                let is_bool = b.reg_tip == 0 && b.reg_bits.is_some();
                                let is_u16_holding = !is_tu && !is_bool && !matches!(b.reg_tip, 2 | 4 | 5);
                                ui.label(format!("val={}", fmt_binding_live(&b, *v, is_u16_holding)));
                            }
                            if let Some(x) = state.bindings.iter_mut().find(|x| x.reg_id == b.reg_id) {
                                let mut label = x.label_override.clone().unwrap_or_default();
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut label)
                                        .hint_text(if is_image_item(&b) { "image path" } else { "label" })
                                        .desired_width(if is_image_item(&b) { 150.0 } else { 90.0 }),
                                );
                                if resp.changed() {
                                    let t = label.trim().to_string();
                                    x.label_override = if t.is_empty() { None } else { Some(t) };
                                    state.dirty = true;
                                }
                                if is_image_item(&b) {
                                    if ui.small_button("sample").on_hover_text("Use web-safe sample path").clicked() {
                                        x.label_override = Some("ui_images/scheme.png".to_string());
                                        x.fmt.get_or_insert_with(|| "contain".to_string());
                                        x.scale_max = Some(x.scale_max.unwrap_or(1.0).clamp(0.0, 1.0));
                                        state.status = Some("image path set; press Save all to store it".to_string());
                                        state.dirty = true;
                                    }
                                    let mut fit = x.fmt.clone().unwrap_or_else(|| "contain".to_string());
                                    egui::ComboBox::from_id_salt(("image_fit", b.reg_id))
                                        .selected_text(fit.as_str())
                                        .width(80.0)
                                        .show_ui(ui, |ui| {
                                            for mode in ["contain", "cover", "stretch"] {
                                                if ui.selectable_label(fit == mode, mode).clicked() {
                                                    fit = mode.to_string();
                                                }
                                            }
                                        });
                                    if x.fmt.as_deref() != Some(fit.as_str()) {
                                        x.fmt = Some(fit);
                                        state.dirty = true;
                                    }
                                    let mut opacity = x.scale_max.unwrap_or(1.0).clamp(0.0, 1.0);
                                    ui.label("op");
                                    if ui.add(egui::DragValue::new(&mut opacity).range(0.0..=1.0).speed(0.02)).changed() {
                                        x.scale_max = Some(opacity.clamp(0.0, 1.0));
                                        state.dirty = true;
                                    }
                                } else if !b.is_text && b.reg_id > 0 {
                                    let mut fmt = x.fmt.clone().unwrap_or_default();
                                    let fmt_resp = ui.add(
                                        egui::TextEdit::singleline(&mut fmt)
                                            .hint_text("fmt")
                                            .desired_width(52.0),
                                    );
                                    let fmt_resp = fmt_resp.on_hover_text(
                                        "Number format for preview. Supported: 0, 0.0, 0.00, 0.000",
                                    );
                                    if fmt_resp.changed() {
                                        let t = fmt.trim().to_string();
                                        x.fmt = if t.is_empty() { None } else { Some(t) };
                                        state.dirty = true;
                                    }
                                    let mut unit = x.unit.clone().unwrap_or_default();
                                    let unit_resp = ui.add(
                                        egui::TextEdit::singleline(&mut unit)
                                            .hint_text("unit")
                                            .desired_width(52.0),
                                    );
                                    let unit_resp = unit_resp.on_hover_text(
                                        "Unit suffix shown in preview, for example C, bar, %, V",
                                    );
                                    if unit_resp.changed() {
                                        let t = unit.trim().to_string();
                                        x.unit = if t.is_empty() { None } else { Some(t) };
                                        state.dirty = true;
                                    }
                                    const COMPONENT_KINDS: &[&str] = &["auto", "led", "numeric", "bar", "gauge", "setpoint", "button", "trend"];
                                    let kind = x.component_kind.as_deref().unwrap_or("auto");
                                    let reg_id = b.reg_id;
                                    let mut chosen: Option<&'static str> = None;
                                    egui::ComboBox::from_id_salt(("comp_kind", reg_id))
                                        .selected_text(kind)
                                        .width(72.0)
                                        .show_ui(ui, |ui| {
                                            for k in COMPONENT_KINDS {
                                                if ui.selectable_label(kind == *k, *k).clicked() {
                                                    chosen = Some(k);
                                                }
                                            }
                                        });
                                    if let Some(k) = chosen {
                                        component_kind_changes.push((
                                            reg_id,
                                            if k == "auto" { None } else { Some(k.to_string()) },
                                        ));
                                    }
                                }
                                let is_bool = x.reg_tip == 0 && x.reg_bits.is_some();
                                let min_w = if is_bool { 8 } else { 30 };
                                let min_h = if is_bool { 8 } else { 18 };
                                let mut wv = x.w.max(min_w);
                                let mut hv = x.h.max(min_h);
                                if ui.add(egui::DragValue::new(&mut wv).range(min_w..=800)).changed() {
                                    x.w = wv;
                                    state.dirty = true;
                                }
                                if ui.add(egui::DragValue::new(&mut hv).range(min_h..=400)).changed() {
                                    x.h = hv;
                                    state.dirty = true;
                                }
                                if matches!(
                                    x.component_kind.as_deref(),
                                    Some("bar" | "gauge" | "setpoint" | "trend")
                                ) {
                                    let mut scale_max = x.scale_max.unwrap_or(100.0);
                                    ui.label("max");
                                    if ui
                                        .add(egui::DragValue::new(&mut scale_max).range(0.1..=1_000_000.0).speed(1.0))
                                        .changed()
                                    {
                                        x.scale_max = Some(scale_max.max(0.1));
                                        state.dirty = true;
                                    }
                                }
                            }
                            let mut v = state
                                .bindings
                                .iter()
                                .find(|x| x.reg_id == b.reg_id)
                                .map(|x| x.visible)
                                .unwrap_or(true);
                            if ui.checkbox(&mut v, "vis").changed() {
                                if let Some(x) = state.bindings.iter_mut().find(|x| x.reg_id == b.reg_id) {
                                    x.visible = v;
                                    state.dirty = true;
                                }
                            }
                            let mut w = state
                                .bindings
                                .iter()
                                .find(|x| x.reg_id == b.reg_id)
                                .map(|x| x.writable)
                                .unwrap_or(false);
                            if ui
                                .add_enabled(!(b.is_text || b.reg_id <= 0), egui::widgets::Checkbox::new(&mut w, "wr"))
                                .changed()
                            {
                                if let Some(x) = state.bindings.iter_mut().find(|x| x.reg_id == b.reg_id) {
                                    x.writable = w;
                                    state.dirty = true;
                                }
                            }
                            if ui.small_button("Up").clicked() {
                                state.move_binding(b.reg_id, -1);
                            }
                            if ui.small_button("Dn").clicked() {
                                state.move_binding(b.reg_id, 1);
                            }
                            if ui.small_button("X").clicked() {
                                state.remove_binding(b.reg_id);
                            }
                        });
                    }
                    for (reg_id, kind) in component_kind_changes {
                        if let Some(binding) = state.bindings.iter_mut().find(|x| x.reg_id == reg_id) {
                            binding.component_kind = kind;
                            state.dirty = true;
                        }
                    }
                });
                    },
                );
            });
        });

    if state.help_open {
        egui::Window::new("UI Link Editor - справка")
            .open(&mut state.help_open)
            .resizable(true)
            .default_size([860.0, 560.0])
            .show(ctx, |ui| {
                ui.heading("Быстрый старт");
                ui.label("1) В главном окне выберите КПЗ и откройте UI Link Editor.");
                ui.label("2) Выберите шаблон, при необходимости нажмите Link to KPZ.");
                ui.label("3) Задайте Code/Title/Description и нажмите Create window from template.");
                ui.separator();

                ui.heading("Группы и регистры");
                ui.label("4) В колонке Groups отметьте одну или несколько групп, затем нажмите Reload regs by groups.");
                ui.label("5) В колонке Available regs используйте find/group-фильтр.");
                ui.label("6) Add one -> добавляет выбранный регистр; Add all -> добавляет все видимые по фильтру.");
                ui.separator();

                ui.heading("Bindings (правая колонка)");
                ui.label("7) Bindings показывает состав окна: порядок, имена, координаты, размеры и текущие значения.");
                ui.label("8) vis - видимость элемента в UI; wr - разрешение записи (команд) в регистр.");
                ui.label("9) Up/Dn меняют порядок; X удаляет элемент; Add text добавляет текстовый блок.");
                ui.label("10) W/H/Gap + Apply size/Stack V/Stack H - пакетное выравнивание выбранных элементов.");
                ui.label("11) Save all сохраняет bindings/layout в БД. При Unsaved changes есть несохраненные правки.");
                ui.separator();

                ui.heading("Шаблоны");
                ui.label("12) Reload templates обновляет список шаблонов.");
                ui.label("13) Link to KPZ / Unlink from KPZ управляют связью шаблона с текущим КПЗ.");
                ui.label("14) Create window from template создает новое окно и копирует в него layout шаблона.");
                ui.label("15) Template editor mode — прямое редактирование шаблона окна: выберите шаблон (или New template), Save template сохраняет метаданные, Save all — привязки и layout.");
                ui.separator();

                ui.heading("Окна и удаление");
                ui.label("16) Save window сохраняет метаданные уже созданного окна (Code/Title/Description).");
                ui.label("17) Delete window удаляет окно текущего КПЗ и связанные bindings/text items.");
                ui.separator();

                ui.heading("Preview/Layout/Диагностика");
                ui.label("18) Preview показывает живые значения и быстрые действия (команды/запись).");
                ui.label("19) Layout - визуальный редактор позиций и размеров элементов.");
                ui.label("20) TX/RX - трассировка обмена для отладки.");
                ui.label("21) Если видите \"io task already in progress\", дождитесь завершения фоновой операции.");
            });
    }

    actions.extend(show_preview_window(ctx, state));
    actions.extend(show_kp_viewer_window(ctx, state, selected_kpz));
    actions.extend(show_bits16_window(ctx, state));
    show_layout_window(ctx, state);
    show_trace_window(ctx, state);

    state.window_template_open = open;
    state.open =
        state.window_template_open || state.kp_template_open || state.kp_binding_open || state.kp_viewer_open;
    actions
}
