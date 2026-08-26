use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::app::{fmt_unix_ts, Ss5App};
use eframe::egui;
use egui_plot::{CoordinatesFormatter, Corner, Legend, Line, Plot, PlotPoints, Points};

impl Ss5App {
    pub(crate) fn open_graph_window(&mut self) {
        self.graph_open = true;
        self.graph_err = None;
        let groups = self.graph_groups_for_selected_kpz();
        let current_ok = self
            .graph_group_id
            .map(|id| groups.iter().any(|g| g.id == id))
            .unwrap_or(false);
        if !current_ok {
            self.graph_group_id = groups.first().map(|g| g.id);
        }
        self.reload_graph_regs();
    }

    fn reload_graph_regs(&mut self) {
        self.graph_err = None;
        self.graph_selected_regs.clear();
        self.graph_series.clear();
        let Some(group_id) = self.graph_group_id else {
            self.graph_regs.clear();
            return;
        };
        match self.db.get_regs_for_group(group_id) {
            Ok(v) => {
                self.graph_regs = v;
            }
            Err(e) => self.graph_err = Some(format!("get_regs_for_group failed: {e}")),
        }
    }

    fn reload_graph_series(&mut self) {
        self.graph_err = None;
        self.graph_series.clear();
        let Some(kpz_id) = self.selected_kpz else {
            self.graph_err = Some("KPZ не выбран".to_string());
            return;
        };
        let reg_ids: Vec<i32> = self.graph_selected_regs.iter().copied().collect();
        if reg_ids.is_empty() {
            return;
        }
        match self
            .db
            .get_arx_series(kpz_id, &reg_ids, self.graph_limit, self.graph_window_sec)
        {
            Ok(v) => {
                self.graph_series = v;
            }
            Err(e) => self.graph_err = Some(format!("get_arx_series failed: {e}")),
        }
    }

    pub(crate) fn show_graph_window(&mut self, ctx: &egui::Context) {
        if !self.graph_open {
            return;
        }

        let mut open = self.graph_open;
        egui::Window::new("График ARX")
            .open(&mut open)
            .resizable(true)
            .default_size([1220.0, 760.0])
            .min_width(760.0)
            .min_height(480.0)
            .show(ctx, |ui| {
                if let Some(err) = &self.graph_err {
                    ui.colored_label(egui::Color32::RED, err);
                }

                let groups = self.graph_groups_for_selected_kpz();
                ui.horizontal(|ui| {
                    ui.label(format!("KPZ: {}", self.selected_kpz_name()));
                    ui.separator();
                    ui.label("Группа:");
                    let mut group_id = self.graph_group_id;
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
                    egui::ComboBox::from_id_salt("graph_group")
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
                    if group_id != self.graph_group_id {
                        self.graph_group_id = group_id;
                        self.reload_graph_regs();
                    }

                    ui.separator();
                    ui.label("Интервал:");
                    let mut win = self.graph_window_sec;
                    egui::ComboBox::from_id_salt("graph_interval")
                        .selected_text(match win {
                            900 => "15m",
                            3600 => "1h",
                            21600 => "6h",
                            86400 => "24h",
                            604800 => "7d",
                            _ => "custom",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut win, 900, "15m");
                            ui.selectable_value(&mut win, 3600, "1h");
                            ui.selectable_value(&mut win, 21600, "6h");
                            ui.selectable_value(&mut win, 86400, "24h");
                            ui.selectable_value(&mut win, 604800, "7d");
                        });
                    self.graph_window_sec = win;

                    if ui.button("Обновить регистры").clicked() {
                        self.reload_graph_regs();
                    }
                    if ui.button("Показать").clicked() {
                        self.reload_graph_series();
                    }
                    if ui.button("Очистить").clicked() {
                        self.graph_series.clear();
                        self.graph_selected_regs.clear();
                    }
                });

                ui.separator();
                egui::SidePanel::left("graph_regs_panel")
                    .resizable(true)
                    .default_width(300.0)
                    .min_width(180.0)
                    .show_inside(ui, |ui| {
                        ui.heading("Регистры");
                        ui.small(format!("Выбрано: {}", self.graph_selected_regs.len()));
                        egui::ScrollArea::vertical()
                            .id_salt("graph_regs")
                            .max_height(620.0)
                            .show(ui, |ui| {
                                for r in &self.graph_regs {
                                    let mut on = self.graph_selected_regs.contains(&r.id);
                                    let label = format!(
                                        "{}  {}  mb={} tip={} bits={}",
                                        r.id,
                                        r.name,
                                        r.mb,
                                        r.tip,
                                        r.bits
                                            .map(|v| v.to_string())
                                            .unwrap_or_else(|| "-".to_string())
                                    );
                                    if ui.checkbox(&mut on, label).changed() {
                                        if on {
                                            self.graph_selected_regs.insert(r.id);
                                        } else {
                                            self.graph_selected_regs.remove(&r.id);
                                        }
                                    }
                                }
                            });
                    });

                ui.heading("График");
                if self.graph_series.is_empty() {
                    ui.label("Выберите регистры и нажмите «Показать».");
                } else {
                    let avail = ui.available_size();
                    if avail.x < 140.0 || avail.y < 120.0 {
                        ui.label("Область графика слишком мала. Увеличьте окно.");
                        return;
                    }
                    let plot_res = catch_unwind(AssertUnwindSafe(|| {
                        Plot::new(format!("arx_plot_{}", self.graph_window_sec))
                            .legend(Legend::default())
                            .allow_zoom([false, true])
                            .allow_scroll([false, true])
                            .x_axis_formatter(|mark, _| fmt_unix_ts(mark.value, true))
                            .label_formatter(|name, value| {
                                let ts = fmt_unix_ts(value.x, false);
                                if name.is_empty() {
                                    format!("t: {ts}\ny: {:.6}", value.y)
                                } else {
                                    format!("{name}\nt: {ts}\ny: {:.6}", value.y)
                                }
                            })
                            .coordinates_formatter(
                                Corner::LeftTop,
                                CoordinatesFormatter::new(|value, _| {
                                    format!("t: {}\ny: {:.6}", fmt_unix_ts(value.x, false), value.y)
                                }),
                            )
                            .show(ui, |plot_ui| {
                                for s in &self.graph_series {
                                    let points_vec: Vec<[f64; 2]> = s
                                        .points
                                        .iter()
                                        .filter_map(|p| {
                                            let x = p.ts_unix as f64;
                                            let y = p.val_num;
                                            if x.is_finite()
                                                && y.is_finite()
                                                && (946_684_800.0..=4_102_444_800.0).contains(&x)
                                            {
                                                Some([x, y])
                                            } else {
                                                None
                                            }
                                        })
                                        .collect();
                                    if points_vec.is_empty() {
                                        continue;
                                    }
                                    let line_points = PlotPoints::from(points_vec.clone());
                                    let points = PlotPoints::from(points_vec);
                                    let line = Line::new(line_points).name(format!("reg {}", s.reg_id));
                                    plot_ui.line(line);
                                    let dots = Points::new(points).radius(2.5);
                                    plot_ui.points(dots);
                                }
                            });
                    }));
                    if plot_res.is_err() {
                        self.graph_err = Some("Не удалось отрисовать график для этого интервала; попробуйте 24h".to_string());
                        self.graph_series.clear();
                    }
                }
            });
        self.graph_open = open;
    }
}
