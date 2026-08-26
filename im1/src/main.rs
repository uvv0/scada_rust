#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::collections::HashMap;
use std::f32::consts::TAU;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::Local;
use eframe::egui;
use serde::Deserialize;

mod imm;
mod server_async;

const DEFAULT_BIND_IP: &str = "127.0.0.1";
const DEFAULT_BIND_PORT: u16 = 5100;
const DEFAULT_MODEM_START: u16 = 50001;
const DEFAULT_MODEM_END: u16 = u16::MAX;
const HOLDING_F32_BASE_ADDR: u16 = 0x10;
const HOLDING_F32_COUNT: usize = 24;
const SIM12_BASE_ADDR: u16 = 0x20;
const SIM12_COUNT: usize = 12;

type SharedSlot = Arc<Mutex<imm::SlotState>>;
type SharedSlots = Arc<Mutex<HashMap<u16, SharedSlot>>>;

#[derive(Clone, Deserialize)]
pub struct ListenerConfig {
    ip: String,
    port: u16,
    modem_start: u16,
    modem_end: u16,
}

#[derive(Clone, Deserialize)]
struct AppConfig {
    #[serde(default)]
    listener: Vec<ListenerConfig>,
}

impl ListenerConfig {
    fn bind_addr(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }

    fn label(&self) -> String {
        format!(
            "{}:{} | {}..={}",
            self.ip, self.port, self.modem_start, self.modem_end
        )
    }

    fn validate(self) -> Result<Self, String> {
        if self.ip.trim().is_empty() {
            return Err("listener.ip is empty".to_string());
        }
        if self.modem_start > self.modem_end {
            return Err(format!(
                "invalid modem range: {}..={}",
                self.modem_start, self.modem_end
            ));
        }
        Ok(self)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            listener: vec![ListenerConfig {
                ip: DEFAULT_BIND_IP.to_string(),
                port: DEFAULT_BIND_PORT,
                modem_start: DEFAULT_MODEM_START,
                modem_end: DEFAULT_MODEM_END,
            }],
        }
    }
}

impl AppConfig {
    fn validate(self) -> Result<Self, String> {
        if self.listener.is_empty() {
            return Err("config.listener is empty".to_string());
        }
        let mut listeners = Vec::with_capacity(self.listener.len());
        for listener in self.listener {
            listeners.push(listener.validate()?);
        }
        for i in 0..listeners.len() {
            for j in (i + 1)..listeners.len() {
                if listeners[i].bind_addr() == listeners[j].bind_addr() {
                    return Err(format!(
                        "duplicate bind_addr: {}",
                        listeners[i].bind_addr()
                    ));
                }
                let a = &listeners[i];
                let b = &listeners[j];
                if a.modem_start <= b.modem_end && b.modem_start <= a.modem_end {
                    return Err(format!(
                        "overlapping modem ranges: {} and {}",
                        a.label(),
                        b.label()
                    ));
                }
            }
        }
        Ok(Self { listener: listeners })
    }
}

fn apply_ss5_visuals(ctx: &egui::Context) {
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
    });
}

#[derive(Clone, Default)]
struct UiState {
    packets: u64,
    sent_packets: u64,
    err_packets: u64,
    dropped_packets: u64,
    last_peer: String,
    last_rx_unix: u64,
    last_rx_bytes: usize,
    last_rx_status: String,
    last_error: String,
    logs: Vec<String>,
}

struct ListenerRuntime {
    config: ListenerConfig,
    ui: Arc<Mutex<UiState>>,
    slots: SharedSlots,
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hex_dump(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_ui_log(ui: &Arc<Mutex<UiState>>, msg: impl Into<String>) {
    let ts = Local::now().format("%m-%d %H:%M:%S").to_string();
    let mut st = ui.lock().unwrap();
    st.logs.push(format!("[{}] {}", ts, msg.into()));
    const MAX_LOGS: usize = 500;
    if st.logs.len() > MAX_LOGS {
        let drop_n = st.logs.len() - MAX_LOGS;
        st.logs.drain(0..drop_n);
    }
}

fn params_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|x| x.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("im1_params.txt")
}

fn config_path() -> PathBuf {
    if let Some(arg1) = std::env::args_os().nth(1) {
        return PathBuf::from(arg1);
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|x| x.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("im1.toml")
}

fn load_config() -> Result<AppConfig, String> {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str::<AppConfig>(&text)
            .map_err(|e| format!("config parse {}: {e}", path.display()))?
            .validate(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AppConfig::default()),
        Err(e) => Err(format!("config read {}: {e}", path.display())),
    }
}

fn load_params() -> Option<(f32, f32)> {
    let text = std::fs::read_to_string(params_path()).ok()?;
    let mut start = None;
    let mut delta = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("start=") {
            start = v.trim().parse::<f32>().ok();
        } else if let Some(v) = line.strip_prefix("delta=") {
            delta = v.trim().parse::<f32>().ok();
        }
    }
    Some((start?, delta?))
}

fn save_params(start: f32, delta: f32) -> Result<(), String> {
    let text = format!("start={start}\ndelta={delta}\n");
    std::fs::write(params_path(), text).map_err(|e| format!("save params: {e}"))
}

struct SimApp {
    listeners: Vec<ListenerRuntime>,
    selected_listener: usize,
    start_value: f32,
    delta_value: f32,
    sim12_values: [f32; SIM12_COUNT],
    sim12_inputs: [String; SIM12_COUNT],
    sim12_sine_enabled: bool,
    sim12_sine_step_ms: u64,
    sim12_sine_max_span: f32,
    sim12_sine_started_at: Option<Instant>,
    sim12_sine_last_step: Option<u64>,
    holding24_values: [f32; HOLDING_F32_COUNT],
    holding24_inputs: [String; HOLDING_F32_COUNT],
}

impl SimApp {
    fn new(listeners: Vec<ListenerRuntime>, start: f32, delta: f32) -> Self {
        let mut app = Self {
            listeners,
            selected_listener: 0,
            start_value: start,
            delta_value: delta,
            sim12_values: [0.0_f32; SIM12_COUNT],
            sim12_inputs: std::array::from_fn(|_| String::new()),
            sim12_sine_enabled: false,
            sim12_sine_step_ms: 500,
            sim12_sine_max_span: 100.0,
            sim12_sine_started_at: None,
            sim12_sine_last_step: None,
            holding24_values: [0.0_f32; HOLDING_F32_COUNT],
            holding24_inputs: std::array::from_fn(|_| String::new()),
        };
        app.reload_inputs_from_selected();
        app
    }

    fn selected_runtime(&self) -> &ListenerRuntime {
        &self.listeners[self.selected_listener]
    }

    fn base_slot(&self) -> SharedSlot {
        let listener = self.selected_runtime();
        listener
            .slots
            .lock()
            .unwrap()
            .get(&listener.config.modem_start)
            .expect("base slot must exist")
            .clone()
    }

    fn reload_inputs_from_selected(&mut self) {
        let base_slot = self.base_slot();
        let slot = base_slot.lock().unwrap();
        for (i, value) in self.sim12_values.iter_mut().enumerate() {
            let addr = SIM12_BASE_ADDR + (i as u16) * 2;
            *value = slot.rd_float(addr);
            self.sim12_inputs[i] = format!("{:.6}", *value);
        }
        for (i, value) in self.holding24_values.iter_mut().enumerate() {
            let addr = HOLDING_F32_BASE_ADDR + (i as u16) * 2;
            *value = slot.rd_float_holding(addr);
            self.holding24_inputs[i] = format!("{:.6}", *value);
        }
        let (start, delta) = slot.gen_params();
        self.start_value = start;
        self.delta_value = delta;
    }

    fn restart_sim12_sine(&mut self) {
        self.sim12_sine_started_at = Some(Instant::now());
        self.sim12_sine_last_step = None;
    }

    fn tick_sim12_sine(&mut self, base_slot: &SharedSlot) {
        if !self.sim12_sine_enabled {
            self.sim12_sine_started_at = None;
            self.sim12_sine_last_step = None;
            return;
        }

        let started_at = *self.sim12_sine_started_at.get_or_insert_with(Instant::now);
        let step_ms = self.sim12_sine_step_ms.max(1);
        let elapsed_ms = Instant::now().duration_since(started_at).as_millis() as u64;
        let step_index = elapsed_ms / step_ms;
        if self.sim12_sine_last_step == Some(step_index) {
            return;
        }
        self.sim12_sine_last_step = Some(step_index);

        let max_value = self.sim12_sine_max_span.abs();
        let amplitude = max_value * 0.5;
        let offset = amplitude;
        let phase_base = (step_index as f32) * (TAU / SIM12_COUNT as f32);
        let mut slot = base_slot.lock().unwrap();
        for i in 0..SIM12_COUNT {
            let addr = SIM12_BASE_ADDR + (i as u16) * 2;
            let phase = phase_base + (i as f32) * (TAU / SIM12_COUNT as f32);
            let value = offset + amplitude * phase.sin();
            self.sim12_values[i] = value;
            self.sim12_inputs[i] = format!("{:.6}", value);
            slot.wr_float(value, addr);
        }
    }
}

impl eframe::App for SimApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_ss5_visuals(ctx);

        let total_packets: u64 = self
            .listeners
            .iter()
            .map(|listener| listener.ui.lock().unwrap().packets)
            .sum();

        egui::TopBottomPanel::bottom("console_panel")
            .resizable(true)
            .default_height(150.0)
            .min_height(90.0)
            .show(ctx, |ui| {
                let selected_state = self.selected_runtime().ui.lock().unwrap().clone();
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Console: {}",
                        self.selected_runtime().config.label()
                    ));
                    if ui.button("Clear").clicked() {
                        let mut s = self.selected_runtime().ui.lock().unwrap();
                        s.logs.clear();
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("im1_console_scroll")
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if selected_state.logs.is_empty() {
                            ui.label("-");
                        } else {
                            for line in &selected_state.logs {
                                ui.label(
                                    egui::RichText::new(line)
                                        .family(egui::FontFamily::Monospace)
                                        .size(12.0),
                                );
                            }
                        }
                    });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let (start_addr, idx_addr, recs, floats, rec_words) = imm::ring_config();
            let selected_ui = self.selected_runtime().ui.clone();
            let base_slot = self.base_slot();
            self.tick_sim12_sine(&base_slot);

            ui.heading("IM1 Ring Archive Simulator");
            ui.separator();
            ui.label(format!(
                "Listeners: {} total_rx={}",
                self.listeners.len(),
                total_packets
            ));

            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    ui.heading("Listeners");
                    ui.separator();
                    let mut new_selected = self.selected_listener;
                    for (idx, listener) in self.listeners.iter().enumerate() {
                        let state = listener.ui.lock().unwrap().clone();
                        let text = format!(
                            "{} | rx={} tx={} err={} drop={} | status={}",
                            listener.config.label(),
                            state.packets,
                            state.sent_packets,
                            state.err_packets,
                            state.dropped_packets,
                            if state.last_rx_status.is_empty() {
                                "-"
                            } else {
                                &state.last_rx_status
                            }
                        );
                        if ui
                            .selectable_label(self.selected_listener == idx, text)
                            .clicked()
                        {
                            new_selected = idx;
                        }
                    }
                    if new_selected != self.selected_listener {
                        self.selected_listener = new_selected;
                        self.reload_inputs_from_selected();
                    }
                });

                cols[1].vertical(|ui| {
                    let listener = self.selected_runtime();
                    let state = listener.ui.lock().unwrap().clone();
                    let base_slot = self.base_slot();
                    let (cur_idx, cur_val, last_minute) = base_slot.lock().unwrap().ring_status();

                    ui.heading("Selected Listener");
                    ui.separator();
                    ui.label(format!("Bind: {}", listener.config.bind_addr()));
                    ui.label(format!(
                        "Allowed modem range: {}..={} ({} slots)",
                        listener.config.modem_start,
                        listener.config.modem_end,
                        u32::from(listener.config.modem_end)
                            - u32::from(listener.config.modem_start)
                            + 1
                    ));
                    ui.label(format!(
                        "Counters: rx={} tx={} err={} drop={}",
                        state.packets,
                        state.sent_packets,
                        state.err_packets,
                        state.dropped_packets
                    ));
                    ui.label(format!(
                        "Last RX: ts={} bytes={} status={} peer={}",
                        state.last_rx_unix,
                        state.last_rx_bytes,
                        state.last_rx_status,
                        state.last_peer
                    ));
                    if !state.last_error.is_empty() {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            format!("Last error: {}", state.last_error),
                        );
                    }

                    ui.separator();
                    ui.label(format!(
                        "Ring: start={} index@{} records={} record_words={} floats={}",
                        start_addr, idx_addr, recs, rec_words, floats
                    ));
                    ui.label(format!(
                        "Current: index={} next_value={:.4} last_minute={}",
                        cur_idx, cur_val, last_minute
                    ));

                    let reg0_now = base_slot.lock().unwrap().reg_u16(0);
                    ui.separator();
                    ui.label(format!("Register 0: 0x{:04X}", reg0_now));
                    let mut bits_value = reg0_now;
                    let mut bits_changed = false;
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            egui::Grid::new("reg0_bits_edit_grid").show(ui, |ui| {
                                for bit in 0..16 {
                                    let mut on = ((bits_value >> bit) & 1) != 0;
                                    if ui.checkbox(&mut on, format!("b{}", bit)).changed() {
                                        if on {
                                            bits_value |= 1u16 << bit;
                                        } else {
                                            bits_value &= !(1u16 << bit);
                                        }
                                        bits_changed = true;
                                    }
                                    if bit % 8 == 7 {
                                        ui.end_row();
                                    }
                                }
                            });
                        });
                        ui.add_space(24.0);
                        ui.vertical(|ui| {
                            ui.label("ТУ статус:");
                            if let Some((addr, on)) = base_slot.lock().unwrap().last_fc05_status() {
                                let color = if on {
                                    egui::Color32::from_rgb(80, 220, 120)
                                } else {
                                    egui::Color32::from_rgb(230, 90, 90)
                                };
                                ui.colored_label(
                                    color,
                                    format!("ТУ адрес {}: {}", addr, if on { "ВКЛ" } else { "ОТКЛ" }),
                                );
                            } else {
                                ui.label("ТУ: -");
                            }
                        });
                    });
                    if bits_changed {
                        base_slot.lock().unwrap().set_reg_u16(0, bits_value);
                        push_ui_log(
                            &listener.ui,
                            format!("reg0 set to {} (0x{:04X})", bits_value, bits_value),
                        );
                    }
                });
            });

            ui.separator();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("Simulation input floats (addr 0x20, 12 values):");
                    ui.horizontal(|ui| {
                        let toggle_label = if self.sim12_sine_enabled {
                            "Stop sine"
                        } else {
                            "Start sine"
                        };
                        if ui.button(toggle_label).clicked() {
                            self.sim12_sine_enabled = !self.sim12_sine_enabled;
                            self.restart_sim12_sine();
                            push_ui_log(
                                &selected_ui,
                                format!(
                                    "input sine {} step_ms={} max_span={:.3}",
                                    if self.sim12_sine_enabled { "ON" } else { "OFF" },
                                    self.sim12_sine_step_ms,
                                    self.sim12_sine_max_span
                                ),
                            );
                        }
                        if ui.button("Restart phase").clicked() {
                            self.restart_sim12_sine();
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Step ms:");
                        if ui
                            .add(egui::DragValue::new(&mut self.sim12_sine_step_ms).range(1..=60_000))
                            .changed()
                            && self.sim12_sine_enabled
                        {
                            self.restart_sim12_sine();
                        }
                        ui.label("Max value:");
                        if ui
                            .add(egui::DragValue::new(&mut self.sim12_sine_max_span).speed(0.1))
                            .changed()
                            && self.sim12_sine_enabled
                        {
                            self.restart_sim12_sine();
                        }
                    });
                    egui::Grid::new("sim12_float_grid").show(ui, |ui| {
                        for i in 0..SIM12_COUNT {
                            let addr = SIM12_BASE_ADDR + (i as u16) * 2;
                            ui.label(format!("0x{:04X}", addr));
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut self.sim12_inputs[i])
                                    .desired_width(90.0)
                                    .interactive(!self.sim12_sine_enabled),
                            );
                            if resp.changed() && !self.sim12_sine_enabled {
                                if let Ok(v) = self.sim12_inputs[i].trim().parse::<f32>() {
                                    self.sim12_values[i] = v;
                                    base_slot.lock().unwrap().wr_float(v, addr);
                                    push_ui_log(
                                        &selected_ui,
                                        format!("input float[{}] addr=0x{:04X} set to {:.6}", i, addr, v),
                                    );
                                }
                            }
                            if !self.sim12_sine_enabled && !resp.has_focus() {
                                let live = base_slot.lock().unwrap().rd_float(addr);
                                if self.sim12_values[i].to_bits() != live.to_bits() {
                                    self.sim12_values[i] = live;
                                    self.sim12_inputs[i] = format!("{:.6}", live);
                                }
                            }
                            if i % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
                });

                ui.add_space(16.0);

                ui.vertical(|ui| {
                    ui.label(format!(
                        "Holding f32 (addr 0x{:04X}, 24 values):",
                        HOLDING_F32_BASE_ADDR
                    ));
                    egui::Grid::new("holding24_float_grid").show(ui, |ui| {
                        for i in 0..HOLDING_F32_COUNT {
                            let addr = HOLDING_F32_BASE_ADDR + (i as u16) * 2;
                            ui.label(format!("0x{:04X}", addr));
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut self.holding24_inputs[i]).desired_width(90.0),
                            );
                            if resp.changed() {
                                if let Ok(v) = self.holding24_inputs[i].trim().parse::<f32>() {
                                    self.holding24_values[i] = v;
                                    base_slot.lock().unwrap().wr_float_holding(v, addr);
                                    push_ui_log(
                                        &selected_ui,
                                        format!("holding float[{}] addr=0x{:04X} set to {:.6}", i, addr, v),
                                    );
                                }
                            }
                            if !resp.has_focus() {
                                let live = base_slot.lock().unwrap().rd_float_holding(addr);
                                if self.holding24_values[i].to_bits() != live.to_bits() {
                                    self.holding24_values[i] = live;
                                    self.holding24_inputs[i] = format!("{:.6}", live);
                                }
                            }
                            if i % 4 == 3 {
                                ui.end_row();
                            }
                        }
                    });
                });
            });

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Start value:");
                ui.add(egui::DragValue::new(&mut self.start_value).speed(0.1));
            });
            ui.horizontal(|ui| {
                ui.label("Delta per minute:");
                ui.add(egui::DragValue::new(&mut self.delta_value).speed(0.01));
            });

            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    base_slot
                        .lock()
                        .unwrap()
                        .set_gen_params(self.start_value, self.delta_value, false);
                    if let Err(err) = save_params(self.start_value, self.delta_value) {
                        selected_ui.lock().unwrap().last_error = err;
                    }
                }
                if ui.button("Apply + Reset Ring").clicked() {
                    base_slot
                        .lock()
                        .unwrap()
                        .set_gen_params(self.start_value, self.delta_value, true);
                    if let Err(err) = save_params(self.start_value, self.delta_value) {
                        selected_ui.lock().unwrap().last_error = err;
                    }
                }
                if ui.button("Tick now").clicked() {
                    base_slot.lock().unwrap().tick_ring_archive_now();
                }
            });

            ui.separator();
            ui.label("Last 5 ring records:");
            egui::Grid::new("last_records_grid").show(ui, |ui| {
                ui.label("Index");
                ui.label("ts");
                ui.label("value0");
                ui.end_row();
                for rec in base_slot.lock().unwrap().last_records(5) {
                    ui.label(rec.index.to_string());
                    ui.label(rec.ts_unix.to_string());
                    ui.label(format!("{:.4}", rec.value0));
                    ui.end_row();
                }
            });
        });

        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

fn main() -> Result<(), eframe::Error> {
    let config = load_config().map_err(|e| {
        eframe::Error::AppCreation(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            e,
        )))
    })?;

    let template_slot = imm::SlotState::new();
    let (start, delta) = load_params().unwrap_or_else(|| template_slot.gen_params());

    let mut listeners = Vec::with_capacity(config.listener.len());
    for listener_config in config.listener {
        let mut base_slot = imm::SlotState::new();
        base_slot.set_gen_params(start, delta, false);
        base_slot.init_ring_archive();

        let mut slot_map = HashMap::new();
        slot_map.insert(listener_config.modem_start, Arc::new(Mutex::new(base_slot)));

        let slots: SharedSlots = Arc::new(Mutex::new(slot_map));
        let ui_state = Arc::new(Mutex::new(UiState::default()));

        let ui_for_thread = ui_state.clone();
        let slots_for_thread = slots.clone();
        let config_for_thread = listener_config.clone();
        thread::spawn(move || {
            let ui_for_server = ui_for_thread.clone();
            if let Err(e) =
                server_async::run_udp_server(ui_for_server, slots_for_thread, config_for_thread.clone())
            {
                let mut st = ui_for_thread.lock().unwrap();
                st.last_error = format!("udp server failed: {e}");
                st.logs.push(format!(
                    "[{}] ERROR udp server {} failed: {}",
                    Local::now().format("%m-%d %H:%M:%S"),
                    config_for_thread.label(),
                    e
                ));
            }
        });

        listeners.push(ListenerRuntime {
            config: listener_config,
            ui: ui_state,
            slots,
        });
    }

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "im1 simulator",
        native_options,
        Box::new(move |cc| {
            apply_ss5_visuals(&cc.egui_ctx);
            Ok(Box::new(SimApp::new(listeners, start, delta)))
        }),
    )
}
