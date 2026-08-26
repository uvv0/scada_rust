use crate::app::{fmt_num_compact, fmt_unix_ts, Ss5App};
use eframe::egui;

impl Ss5App {
    pub(crate) fn open_arx_val_window(&mut self) {
        self.arx_val_err = None;
        self.arx_val_status = None;
        self.arx_val_kpz_filter = self.selected_kpz.map(|v| v.to_string()).unwrap_or_default();
        self.reload_arx_val_rows();
        self.arx_val_open = true;
    }

    fn reload_arx_val_rows(&mut self) {
        let kpz_filter = {
            let s = self.arx_val_kpz_filter.trim();
            if s.is_empty() {
                None
            } else {
                match s.parse::<i32>() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        self.arx_val_err = Some("Фильтр kpz_id должен быть целым числом".to_string());
                        return;
                    }
                }
            }
        };
        match self.db.get_last_arx_val_rows(kpz_filter, self.arx_val_limit) {
            Ok(v) => {
                self.arx_val_rows = v;
                self.arx_val_err = None;
                self.arx_val_status = Some(format!("Загружено строк: {}", self.arx_val_rows.len()));
            }
            Err(e) => {
                self.arx_val_err = Some(format!("get_last_arx_val_rows failed: {e}"));
            }
        }
    }

    pub(crate) fn show_arx_val_window(&mut self, ctx: &egui::Context) {
        if !self.arx_val_open {
            return;
        }

        let mut open = self.arx_val_open;
        egui::Window::new("Значения ARX")
            .open(&mut open)
            .resizable(true)
            .default_size([920.0, 460.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Фильтр kpz_id:");
                    ui.add(egui::TextEdit::singleline(&mut self.arx_val_kpz_filter).desired_width(100.0));
                    if ui.button("Обновить").clicked() {
                        self.reload_arx_val_rows();
                    }
                    if ui.button("Еще +20").clicked() {
                        self.arx_val_limit += 20;
                        self.reload_arx_val_rows();
                    }
                    ui.label(format!("limit={}", self.arx_val_limit));
                });

                if let Some(err) = &self.arx_val_err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &self.arx_val_status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }
                ui.separator();

                egui::ScrollArea::vertical()
                    .id_salt("arx_val_rows")
                    .show(ui, |ui| {
                        if self.arx_val_rows.is_empty() {
                            ui.label("-");
                        }
                        for row in &self.arx_val_rows {
                            let val_text = row
                                .val_num
                                .map(fmt_num_compact)
                                .unwrap_or_else(|| "null".to_string());
                            let ts_text = fmt_unix_ts(row.ts_unix as f64, false);
                            let text = format!(
                                "{}  id={}  kpz={}  reg={}  tip={}  val={}  ts={} (unix={})",
                                row.created_at,
                                row.id,
                                row.kpz_id,
                                row.reg_id,
                                row.tip,
                                val_text,
                                ts_text,
                                row.ts_unix
                            );
                            ui.label(
                                egui::RichText::new(text).family(egui::FontFamily::Monospace),
                            );
                        }
                    });
            });
        self.arx_val_open = open;
    }
}
