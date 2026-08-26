use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;

use crate::models::{GroupRow, RegRow, UiKpzWindowRow, UiWindowBindingRow};

#[derive(Clone, Debug)]
pub struct UiLinkEditorState {
    pub open: bool,
    pub windows: Vec<UiKpzWindowRow>,
    pub selected_window_id: Option<i64>,
    pub window_code: String,
    pub window_title: String,
    pub window_description: String,
    pub groups_selected: BTreeSet<i32>,
    pub regs_available: Vec<RegRow>,
    pub regs_selected: BTreeSet<i32>,
    pub bindings: Vec<UiWindowBindingRow>,
    pub status: Option<String>,
    pub err: Option<String>,
    pub dirty: bool,
    pub live_values: BTreeMap<i32, Option<f64>>,
    pub cmd_inputs: BTreeMap<i32, String>,
    pub last_cmd_result: BTreeMap<i32, String>,
    pub preview_edit_reg_id: Option<i32>,
}

impl Default for UiLinkEditorState {
    fn default() -> Self {
        Self {
            open: false,
            windows: Vec::new(),
            selected_window_id: None,
            window_code: String::new(),
            window_title: String::new(),
            window_description: String::new(),
            groups_selected: BTreeSet::new(),
            regs_available: Vec::new(),
            regs_selected: BTreeSet::new(),
            bindings: Vec::new(),
            status: None,
            err: None,
            dirty: false,
            live_values: BTreeMap::new(),
            cmd_inputs: BTreeMap::new(),
            last_cmd_result: BTreeMap::new(),
            preview_edit_reg_id: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum UiLinkEditorAction {
    ReloadWindows,
    SelectWindow(Option<i64>),
    UpsertWindow,
    ReloadRegs,
    SaveAll,
    PollNow,
    SendTu { reg_id: i32, on: bool },
    WriteValue { reg_id: i32, val: f64 },
}

fn fmt_live(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{x:.3}"),
        None => "-".to_string(),
    }
}

fn is_tu_binding(b: &UiWindowBindingRow) -> bool {
    b.reg_n_mb == 1 || b.reg_tip == 1
}

fn is_bool_binding(b: &UiWindowBindingRow) -> bool {
    b.reg_tip == 0 && b.reg_bits.is_some()
}

fn is_word16_binding(b: &UiWindowBindingRow) -> bool {
    !is_tu_binding(b) && !is_bool_binding(b) && !matches!(b.reg_tip, 2 | 4 | 5)
}

fn preview_edit_seed(live: Option<f64>, as_word16: bool) -> String {
    if as_word16 {
        live.map(|v| format!("{}", v.round().clamp(0.0, 65535.0) as i64))
            .unwrap_or_default()
    } else {
        live.map(|v| v.to_string()).unwrap_or_default()
    }
}

fn binding_rect(canvas: egui::Rect, b: &UiWindowBindingRow) -> egui::Rect {
    let is_bool = is_bool_binding(b);
    let min_w = if is_bool { 8 } else { 30 };
    let min_h = if is_bool { 8 } else { 18 };
    let top_left = canvas.min + egui::vec2(b.x as f32, b.y as f32);
    egui::Rect::from_min_size(
        top_left,
        egui::vec2(b.w.max(min_w) as f32, b.h.max(min_h) as f32),
    )
}

pub fn show_ui_link_editor(
    ctx: &egui::Context,
    state: &mut UiLinkEditorState,
    selected_kpz: Option<i32>,
    selected_kpz_name: &str,
    _groups: &[GroupRow],
) -> Vec<UiLinkEditorAction> {
    let mut actions = Vec::new();
    if !state.open {
        return actions;
    }

    let mut open = state.open;
    egui::Window::new("KPZ Preview")
        .open(&mut open)
        .resizable(true)
        .default_size([980.0, 640.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "KPZ: {} {}",
                    selected_kpz
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "<none>".to_string()),
                    selected_kpz_name
                ));
                if ui.button("Reload windows").clicked() {
                    actions.push(UiLinkEditorAction::ReloadWindows);
                }
                let mut selected = state.selected_window_id;
                let selected_text = selected
                    .and_then(|id| state.windows.iter().find(|w| w.id == id))
                    .map(|w| format!("{} [{}]", w.title, w.code))
                    .unwrap_or_else(|| "<select window>".to_string());
                egui::ComboBox::from_id_salt("ss5_ui_preview_window_select")
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        for w in &state.windows {
                            ui.selectable_value(
                                &mut selected,
                                Some(w.id),
                                format!("{} [{}]", w.title, w.code),
                            );
                        }
                    });
                if selected != state.selected_window_id {
                    actions.push(UiLinkEditorAction::SelectWindow(selected));
                }
                if ui.button("Сосчитать").clicked() {
                    actions.push(UiLinkEditorAction::PollNow);
                }
            });
            ui.horizontal(|ui| {
                ui.label("ЛКМ: TU OFF/ON, запись bool, ввод значения.");
                ui.label("U16 показывается и редактируется как целое.");
            });

            if let Some(err) = &state.err {
                ui.colored_label(egui::Color32::RED, err);
            } else if let Some(msg) = &state.status {
                ui.colored_label(egui::Color32::GREEN, msg);
            }
            ui.separator();

            let mut rows = state.bindings.clone();
            rows.retain(|b| b.visible);
            rows.sort_by_key(|b| (b.pos, b.reg_id));

            if rows.is_empty() {
                ui.label("No visible bindings in selected window.");
                return;
            }

            let canvas_size = egui::vec2(ui.available_width(), ui.available_height().max(300.0));
            let (rect, resp) = ui.allocate_exact_size(canvas_size, egui::Sense::click());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(16, 18, 30));
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(80)),
            );

            for b in &rows {
                let r = binding_rect(rect, b);
                let is_tu = is_tu_binding(b);
                let is_bool = is_bool_binding(b);
                let is_word16 = is_word16_binding(b);

                if is_tu {
                    let mid_x = (r.min.x + r.max.x) * 0.5;
                    let left = egui::Rect::from_min_max(r.min, egui::pos2(mid_x, r.max.y));
                    let right = egui::Rect::from_min_max(egui::pos2(mid_x, r.min.y), r.max);
                    painter.rect_filled(left, 3.0, egui::Color32::from_rgb(170, 48, 48));
                    painter.rect_filled(right, 3.0, egui::Color32::from_rgb(48, 150, 64));
                } else {
                    painter.rect_filled(r, 3.0, egui::Color32::from_rgb(58, 88, 164));
                }
                painter.rect_stroke(r, 3.0, egui::Stroke::new(1.0, egui::Color32::WHITE));

                let lead = b
                    .label_override
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&b.reg_name);
                painter.text(
                    r.left_center() + egui::vec2(-6.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    lead,
                    egui::TextStyle::Body.resolve(ui.style()),
                    egui::Color32::LIGHT_GRAY,
                );

                let live = *state.live_values.get(&b.reg_id).unwrap_or(&None);
                if is_tu {
                    painter.text(
                        r.center(),
                        egui::Align2::CENTER_CENTER,
                        "OFF | ON",
                        egui::TextStyle::Body.resolve(ui.style()),
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
                } else {
                    let is_edit = b.writable && state.preview_edit_reg_id == Some(b.reg_id);
                    if is_edit {
                        let e = state
                            .cmd_inputs
                            .entry(b.reg_id)
                            .or_insert_with(|| preview_edit_seed(live, is_word16));
                        let edit_rect = egui::Rect::from_min_max(
                            egui::pos2(r.min.x + 6.0, r.min.y + 24.0),
                            egui::pos2(r.max.x - 6.0, r.max.y - 6.0),
                        );
                        let inner =
                            ui.scope_builder(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(e)
                                        .desired_width(edit_rect.width() - 4.0),
                                )
                            });
                        let resp_edit = inner.inner;
                        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if resp_edit.lost_focus() && enter {
                            let parsed = if is_word16 {
                                e.trim()
                                    .parse::<i64>()
                                    .ok()
                                    .map(|v| v.clamp(0, 65535) as f64)
                            } else {
                                e.trim().replace(',', ".").parse::<f64>().ok()
                            };
                            if let Some(v) = parsed {
                                actions.push(UiLinkEditorAction::WriteValue {
                                    reg_id: b.reg_id,
                                    val: v,
                                });
                            } else {
                                state.err = Some(format!("bad value for reg {}", b.reg_id));
                            }
                            state.preview_edit_reg_id = None;
                        }
                    } else {
                        let v = if is_word16 {
                            live.map(|x| format!("{}", x.round().clamp(0.0, 65535.0) as i64))
                                .unwrap_or_else(|| "-".to_string())
                        } else {
                            fmt_live(live)
                        };
                        let mark = if b.writable { " [wr]" } else { "" };
                        painter.text(
                            r.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}{}", v, mark),
                            egui::TextStyle::Body.resolve(ui.style()),
                            egui::Color32::WHITE,
                        );
                    }
                }

                if let Some(mark) = state.last_cmd_result.get(&b.reg_id) {
                    let color = if mark.starts_with("OK") {
                        egui::Color32::from_rgb(80, 220, 120)
                    } else {
                        egui::Color32::from_rgb(240, 90, 90)
                    };
                    painter.text(
                        r.right_top() + egui::vec2(-4.0, 4.0),
                        egui::Align2::RIGHT_TOP,
                        mark,
                        egui::TextStyle::Small.resolve(ui.style()),
                        color,
                    );
                }
            }

            if resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    for b in rows.iter().rev() {
                        let r = binding_rect(rect, b);
                        if !r.contains(pos) {
                            continue;
                        }
                        if is_tu_binding(b) {
                            let on = pos.x >= r.center().x;
                            actions.push(UiLinkEditorAction::SendTu {
                                reg_id: b.reg_id,
                                on,
                            });
                        } else if b.writable {
                            if is_bool_binding(b) {
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
                                state.preview_edit_reg_id = None;
                            } else {
                                let live = state.live_values.get(&b.reg_id).copied().flatten();
                                let is_word16 = is_word16_binding(b);
                                state.preview_edit_reg_id = Some(b.reg_id);
                                state
                                    .cmd_inputs
                                    .entry(b.reg_id)
                                    .or_insert_with(|| preview_edit_seed(live, is_word16));
                                if is_word16 {
                                    state.status = Some(format!("reg {}: direct edit mode", b.reg_id));
                                }
                            }
                        } else {
                            state.preview_edit_reg_id = None;
                        }
                        break;
                    }
                }
            }
        });

    state.open = open;
    actions
}
