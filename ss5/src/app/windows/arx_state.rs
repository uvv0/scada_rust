use crate::app::Ss5App;
use eframe::egui;

impl Ss5App {
    pub(crate) fn open_arx_state_window(&mut self) {
        self.arx_state_err = None;
        self.arx_state_status = None;
        self.arx_state_kpz_filter = self.selected_kpz.map(|v| v.to_string()).unwrap_or_default();
        self.arx_state_kpz_id = self.selected_kpz.map(|v| v.to_string()).unwrap_or_default();
        self.arx_state_arx_id.clear();
        self.arx_state_last_ind.clear();
        self.reload_arx_state_rows();
        self.arx_state_open = true;
    }

    fn reload_arx_state_rows(&mut self) {
        let kpz_filter = {
            let s = self.arx_state_kpz_filter.trim();
            if s.is_empty() {
                None
            } else {
                match s.parse::<i32>() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        self.arx_state_err = Some("Фильтр kpz_id должен быть целым числом".to_string());
                        return;
                    }
                }
            }
        };
        match self.db.get_arx_state_rows(kpz_filter, self.arx_state_limit) {
            Ok(v) => {
                self.arx_state_rows = v;
                self.arx_state_err = None;
            }
            Err(e) => {
                self.arx_state_err = Some(format!("get_arx_state_rows failed: {e}"));
            }
        }
    }

    fn arx_state_filter_kpz_id(&mut self) -> Option<i32> {
        let s = self.arx_state_kpz_filter.trim();
        if s.is_empty() {
            return None;
        }
        match s.parse::<i32>() {
            Ok(v) => Some(v),
            Err(_) => {
                self.arx_state_err = Some("Фильтр kpz_id должен быть целым числом".to_string());
                None
            }
        }
    }

    fn clear_arx_val_from_window(&mut self) {
        let kpz_filter = self.arx_state_filter_kpz_id();
        if self.arx_state_err.is_some() && self.arx_state_kpz_filter.trim().parse::<i32>().is_err() {
            return;
        }
        match self.db.clear_arx_val(kpz_filter) {
            Ok(n) => {
                self.arx_state_status = Some(format!("arx_val очищен: {}", n));
                self.arx_state_err = None;
                self.push_log(format!("ARX state window: cleared arx_val rows={}", n));
            }
            Err(e) => self.arx_state_err = Some(format!("clear_arx_val failed: {e}")),
        }
    }

    fn clear_elam_from_window(&mut self) {
        let kpz_filter = self.arx_state_filter_kpz_id();
        if self.arx_state_err.is_some() && self.arx_state_kpz_filter.trim().parse::<i32>().is_err() {
            return;
        }
        match self.db.clear_elam(kpz_filter) {
            Ok(n) => {
                self.arx_state_status = Some(format!("elam очищен: {}", n));
                self.arx_state_err = None;
                self.reload_logs();
                self.push_log(format!("ARX state window: cleared elam rows={}", n));
            }
            Err(e) => self.arx_state_err = Some(format!("clear_elam failed: {e}")),
        }
    }

    fn clear_poll_log_from_window(&mut self) {
        let kpz_filter = self.arx_state_filter_kpz_id();
        if self.arx_state_err.is_some() && self.arx_state_kpz_filter.trim().parse::<i32>().is_err() {
            return;
        }
        match self.db.clear_poll_log(kpz_filter) {
            Ok(n) => {
                self.arx_state_status = Some(format!("poll_log очищен: {}", n));
                self.arx_state_err = None;
                self.reload_logs();
                self.push_log(format!("ARX state window: cleared poll_log rows={}", n));
            }
            Err(e) => self.arx_state_err = Some(format!("clear_poll_log failed: {e}")),
        }
    }

    fn clear_all_from_window(&mut self) {
        let n_arx = match self.db.clear_arx_val(None) {
            Ok(n) => n,
            Err(e) => {
                self.arx_state_err = Some(format!("clear_arx_val failed: {e}"));
                return;
            }
        };
        let n_elam = match self.db.clear_elam(None) {
            Ok(n) => n,
            Err(e) => {
                self.arx_state_err = Some(format!("clear_elam failed: {e}"));
                return;
            }
        };
        let n_poll = match self.db.clear_poll_log(None) {
            Ok(n) => n,
            Err(e) => {
                self.arx_state_err = Some(format!("clear_poll_log failed: {e}"));
                return;
            }
        };
        self.arx_state_err = None;
        self.arx_state_status = Some(format!(
            "Очищено глобально: arx_val={}, elam={}, poll_log={}",
            n_arx, n_elam, n_poll
        ));
        self.reload_logs();
        self.reload_arx_state_rows();
        self.push_log(format!(
            "ARX state window: global clear arx_val={}, elam={}, poll_log={}",
            n_arx, n_elam, n_poll
        ));
    }

    fn save_arx_state_row(&mut self) {
        let kpz_id = match self.arx_state_kpz_id.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.arx_state_err = Some("kpz_id должен быть целым числом".to_string());
                return;
            }
        };
        let arx_id = match self.arx_state_arx_id.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.arx_state_err = Some("arx_id должен быть целым числом".to_string());
                return;
            }
        };
        let last_ind = match self.arx_state_last_ind.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.arx_state_err = Some("last_ind должен быть целым числом".to_string());
                return;
            }
        };
        if let Err(e) = self.db.upsert_arx_state(kpz_id, arx_id, last_ind) {
            self.arx_state_err = Some(format!("upsert_arx_state failed: {e}"));
            return;
        }
        self.arx_state_status = Some("ARX state сохранен".to_string());
        self.arx_state_err = None;
        self.push_log(format!(
            "ARX state upsert: kpz_id={}, arx_id={}, last_ind={}",
            kpz_id, arx_id, last_ind
        ));
        self.reload_arx_state_rows();
    }

    pub(crate) fn show_arx_state_window(&mut self, ctx: &egui::Context) {
        if !self.arx_state_open {
            return;
        }

        let mut open = self.arx_state_open;
        egui::Window::new("Состояние ARX")
            .open(&mut open)
            .resizable(true)
            .default_size([780.0, 420.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Фильтр kpz_id:");
                    ui.add(egui::TextEdit::singleline(&mut self.arx_state_kpz_filter).desired_width(100.0));
                    if ui.button("Обновить").clicked() {
                        self.reload_arx_state_rows();
                    }
                    if ui.button("Еще").clicked() {
                        self.arx_state_limit += 200;
                        self.reload_arx_state_rows();
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("kpz_id:");
                    ui.add(egui::TextEdit::singleline(&mut self.arx_state_kpz_id).desired_width(80.0));
                    ui.label("arx_id:");
                    ui.add(egui::TextEdit::singleline(&mut self.arx_state_arx_id).desired_width(80.0));
                    ui.label("last_ind:");
                    ui.add(egui::TextEdit::singleline(&mut self.arx_state_last_ind).desired_width(110.0));
                    if ui.button("Сохранить").clicked() {
                        self.save_arx_state_row();
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Очистить arx_val").clicked() {
                        self.clear_arx_val_from_window();
                    }
                    if ui.button("Очистить elam").clicked() {
                        self.clear_elam_from_window();
                    }
                    if ui.button("Очистить poll_log").clicked() {
                        self.clear_poll_log_from_window();
                    }
                    if ui.button("Очистить все").clicked() {
                        self.clear_all_from_window();
                    }
                    ui.label("(использует фильтр kpz_id, если он задан)");
                });

                if let Some(err) = &self.arx_state_err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &self.arx_state_status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }
                ui.separator();

                egui::ScrollArea::vertical().id_salt("arx_state_rows").show(ui, |ui| {
                    if self.arx_state_rows.is_empty() {
                        ui.label("-");
                    }
                    for row in &self.arx_state_rows {
                        let text = format!(
                            "{}  kpz={}  arx={}  last_ind={}",
                            row.updated_at, row.kpz_id, row.arx_id, row.last_ind
                        );
                        let clicked = ui
                            .selectable_label(false, egui::RichText::new(text).family(egui::FontFamily::Monospace))
                            .clicked();
                        if clicked {
                            self.arx_state_kpz_id = row.kpz_id.to_string();
                            self.arx_state_arx_id = row.arx_id.to_string();
                            self.arx_state_last_ind = row.last_ind.to_string();
                        }
                    }
                });
            });
        self.arx_state_open = open;
    }
}
