use crate::app::Ss5App;
use eframe::egui;

impl Ss5App {
    pub(crate) fn open_runtime_cfg_window(&mut self) {
        self.runtime_cfg_err = None;
        self.runtime_cfg_status = None;
        self.reload_runtime_cfg_from_db();
        self.runtime_cfg_open = true;
    }

    fn reload_runtime_cfg_from_db(&mut self) {
        match self.db.get_scheduler_runtime_cfg() {
            Ok(cfg) => {
                self.runtime_no_resp_failures = cfg.no_response_failures.to_string();
                self.runtime_no_resp_backoff_sec = cfg.no_response_backoff_sec.to_string();
                self.runtime_metrics_p95_warn_ms = cfg.metrics_p95_warn_ms.to_string();
                self.runtime_metrics_p95_crit_ms = cfg.metrics_p95_crit_ms.to_string();
                self.runtime_modbus_a_timeout_ms = cfg.modbus_a_timeout_ms.to_string();
                self.runtime_modbus_script_timeout_ms = cfg.modbus_script_timeout_ms.to_string();
                self.runtime_cfg_err = None;
            }
            Err(e) => {
                self.runtime_cfg_err = Some(format!("get_scheduler_runtime_cfg failed: {e}"));
            }
        }
    }

    fn save_runtime_cfg_to_db(&mut self) {
        let no_resp_failures = match self.runtime_no_resp_failures.trim().parse::<i32>() {
            Ok(v) if (1..=20).contains(&v) => v,
            _ => {
                self.runtime_cfg_err = Some("no_response_failures должен быть целым числом в диапазоне 1..20".to_string());
                return;
            }
        };
        let no_resp_backoff_sec = match self.runtime_no_resp_backoff_sec.trim().parse::<i64>() {
            Ok(v) if (1..=86_400).contains(&v) => v,
            _ => {
                self.runtime_cfg_err =
                    Some("no_response_backoff_sec должен быть целым числом в диапазоне 1..86400".to_string());
                return;
            }
        };
        let metrics_p95_warn_ms = match self.runtime_metrics_p95_warn_ms.trim().parse::<i64>() {
            Ok(v) if (100..=60_000).contains(&v) => v,
            _ => {
                self.runtime_cfg_err = Some("metrics_p95_warn_ms должен быть целым числом в диапазоне 100..60000".to_string());
                return;
            }
        };
        let metrics_p95_crit_ms = match self.runtime_metrics_p95_crit_ms.trim().parse::<i64>() {
            Ok(v) if (metrics_p95_warn_ms..=120_000).contains(&v) => v,
            _ => {
                self.runtime_cfg_err = Some(
                    "metrics_p95_crit_ms должен быть целым числом в диапазоне metrics_p95_warn_ms..120000".to_string(),
                );
                return;
            }
        };
        let modbus_a_timeout_ms = match self.runtime_modbus_a_timeout_ms.trim().parse::<i64>() {
            Ok(v) if (200..=30_000).contains(&v) => v,
            _ => {
                self.runtime_cfg_err = Some("modbus_a_timeout_ms должен быть целым числом в диапазоне 200..30000".to_string());
                return;
            }
        };
        let modbus_script_timeout_ms =
            match self.runtime_modbus_script_timeout_ms.trim().parse::<i64>() {
                Ok(v) if (200..=30_000).contains(&v) => v,
                _ => {
                    self.runtime_cfg_err =
                        Some("modbus_script_timeout_ms должен быть целым числом в диапазоне 200..30000".to_string());
                    return;
                }
            };
        if let Err(e) = self
            .db
            .upsert_scheduler_runtime_cfg(
                no_resp_failures,
                no_resp_backoff_sec,
                metrics_p95_warn_ms,
                metrics_p95_crit_ms,
                modbus_a_timeout_ms,
                modbus_script_timeout_ms,
            )
        {
            self.runtime_cfg_err = Some(format!("upsert_scheduler_runtime_cfg failed: {e}"));
            return;
        }
        self.runtime_cfg_err = None;
        self.runtime_cfg_status = Some("Параметры runtime сохранены".to_string());
        self.push_log(format!(
            "Scheduler runtime cfg updated: no_response_failures={}, no_response_backoff_sec={}, metrics_p95_warn_ms={}, metrics_p95_crit_ms={}, modbus_a_timeout_ms={}, modbus_script_timeout_ms={}",
            no_resp_failures, no_resp_backoff_sec, metrics_p95_warn_ms, metrics_p95_crit_ms, modbus_a_timeout_ms, modbus_script_timeout_ms
        ));
    }

    pub(crate) fn show_runtime_cfg_window(&mut self, ctx: &egui::Context) {
        if !self.runtime_cfg_open {
            return;
        }

        let mut open = self.runtime_cfg_open;
        egui::Window::new("Параметры runtime")
            .open(&mut open)
            .resizable(true)
            .default_size([650.0, 260.0])
            .show(ctx, |ui| {
                ui.label("Глобальные параметры планировщика ss4, общие для всех KPZ.");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("no_response_failures:");
                    ui.add(egui::TextEdit::singleline(&mut self.runtime_no_resp_failures).desired_width(110.0));
                    ui.label("1..20");
                });
                ui.horizontal(|ui| {
                    ui.label("no_response_backoff_sec:");
                    ui.add(egui::TextEdit::singleline(&mut self.runtime_no_resp_backoff_sec).desired_width(110.0));
                    ui.label("1..86400");
                });
                ui.horizontal(|ui| {
                    ui.label("metrics_p95_warn_ms:");
                    ui.add(egui::TextEdit::singleline(&mut self.runtime_metrics_p95_warn_ms).desired_width(110.0));
                    ui.label("100..60000");
                });
                ui.horizontal(|ui| {
                    ui.label("metrics_p95_crit_ms:");
                    ui.add(egui::TextEdit::singleline(&mut self.runtime_metrics_p95_crit_ms).desired_width(110.0));
                    ui.label("warn..120000");
                });
                ui.horizontal(|ui| {
                    ui.label("modbus_a_timeout_ms:");
                    ui.add(egui::TextEdit::singleline(&mut self.runtime_modbus_a_timeout_ms).desired_width(110.0));
                    ui.label("200..30000");
                });
                ui.horizontal(|ui| {
                    ui.label("modbus_script_timeout_ms:");
                    ui.add(egui::TextEdit::singleline(&mut self.runtime_modbus_script_timeout_ms).desired_width(110.0));
                    ui.label("200..30000");
                });
                ui.label("Поля p95 задают пороги состояния метрик. modbus_*_timeout_ms задают сетевые таймауты.");
                ui.horizontal(|ui| {
                    if ui.button("Обновить").clicked() {
                        self.reload_runtime_cfg_from_db();
                    }
                    if ui.button("Сохранить").clicked() {
                        self.save_runtime_cfg_to_db();
                    }
                });
                if let Some(err) = &self.runtime_cfg_err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &self.runtime_cfg_status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }
            });
        self.runtime_cfg_open = open;
    }
}
