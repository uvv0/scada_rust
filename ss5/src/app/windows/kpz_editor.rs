use crate::app::Ss5App;
use eframe::egui;

impl Ss5App {
    pub(crate) fn open_kpz_editor(&mut self) {
        if self.obj_rows.is_empty() {
            self.reload_kpz_refs();
        }
        self.sync_kpz_full_editor_from_selected();
        self.kpz_editor_open = true;
    }

    fn save_kpz_full(&mut self) {
        let Some(id) = self.selected_kpz else {
            self.kpz_editor_err = Some("KPZ не выбран".to_string());
            return;
        };

        let name = self.kpz_editor_name.trim();
        if name.is_empty() {
            self.kpz_editor_err = Some("Имя обязательно".to_string());
            return;
        }
        let rtu = match self.kpz_editor_rtu.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.kpz_editor_err = Some("RTU должен быть целым числом".to_string());
                return;
            }
        };
        let obj = match self.kpz_editor_obj {
            Some(v) => v,
            None => {
                self.kpz_editor_err = Some("OBJ должен быть выбран".to_string());
                return;
            }
        };
        let modem = {
            let s = self.kpz_editor_modem.trim();
            if s.is_empty() {
                None
            } else {
                match s.parse::<i32>() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        self.kpz_editor_err = Some("modem должен быть целым числом или пустым".to_string());
                        return;
                    }
                }
            }
        };
        let max_pkt_len = match self.kpz_editor_max_pkt_len.trim().parse::<i32>() {
            Ok(v) if (32..=1500).contains(&v) => v,
            Ok(_) => {
                self.kpz_editor_err = Some("max_pkt_len должен быть в диапазоне 32..1500".to_string());
                return;
            }
            Err(_) => {
                self.kpz_editor_err = Some("max_pkt_len должен быть целым числом".to_string());
                return;
            }
        };
        let t_a = {
            let s = self.kpz_editor_t_a.trim();
            if s.is_empty() {
                None
            } else {
                match s.parse::<i32>() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        self.kpz_editor_err = Some("t_a должен быть целым числом или пустым".to_string());
                        return;
                    }
                }
            }
        };
        let t_script = {
            let s = self.kpz_editor_t_script.trim();
            if s.is_empty() {
                None
            } else {
                match s.parse::<i32>() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        self.kpz_editor_err = Some("t_script должен быть целым числом или пустым".to_string());
                        return;
                    }
                }
            }
        };
        let start = if self.kpz_editor_start { 1 } else { 0 };
        let en_post = self.kpz_editor_en_post;

        if let Err(e) = self
            .db
            .update_kpz_full(id, name, rtu, obj, modem, max_pkt_len, start, t_a, t_script, en_post)
        {
            self.kpz_editor_err = Some(format!("update_kpz_full failed: {e}"));
            return;
        }

        if let Some(row) = self.kpz.iter_mut().find(|r| r.id == id) {
            row.name = name.to_string();
            row.rtu = rtu;
            row.obj = obj;
            row.modem = modem;
            row.max_pkt_len = Some(max_pkt_len);
            row.start = start;
            row.en_post = en_post;
            row.t_a = self.kpz_editor_t_a.clone();
            row.t_script = self.kpz_editor_t_script.clone();
        }

        self.start_flag = self.kpz_editor_start;
        self.kpz_t_a_edit = self.kpz_editor_t_a.clone();
        self.kpz_t_script_edit = self.kpz_editor_t_script.clone();
        self.kpz_editor_err = None;
        self.kpz_editor_status = Some("Сохранено".to_string());
        self.kpz_save_status = Some("Сохранено".to_string());
        self.push_log(format!("KPZ {} сохранен", id));
    }

    fn create_kpz_new(&mut self) {
        if self.obj_rows.is_empty() {
            self.reload_kpz_refs();
        }
        let obj_id = self
            .kpz_editor_obj
            .or_else(|| self.obj_rows.first().map(|o| o.id));
        let Some(obj_id) = obj_id else {
            self.kpz_editor_err = Some("OBJ не найден: создать KPZ нельзя".to_string());
            return;
        };

        let forced_id = {
            let s = self.kpz_editor_id.trim();
            if s.is_empty() {
                None
            } else {
                match s.parse::<i32>() {
                    Ok(v) if v > 0 => Some(v),
                    _ => {
                        self.kpz_editor_err = Some("id должен быть положительным целым числом или пустым".to_string());
                        return;
                    }
                }
            }
        };

        let new_id = match self.db.create_kpz_new(obj_id, forced_id) {
            Ok(id) => id,
            Err(e) => {
                self.kpz_editor_err = Some(format!("create_kpz_new failed: {e}"));
                return;
            }
        };

        self.reload_kpz_refs();
        self.selected_kpz = Some(new_id);
        self.sync_kpz_editor_from_selected();
        self.sync_kpz_full_editor_from_selected();
        self.kpz_editor_err = None;
        self.kpz_editor_status = Some(format!("Создан KPZ {}", new_id));
        self.kpz_save_status = Some("Сохранено".to_string());
        self.push_log(format!("KPZ {} создан", new_id));
    }

    pub(crate) fn show_kpz_editor(&mut self, ctx: &egui::Context) {
        if !self.kpz_editor_open {
            return;
        }

        let mut open = self.kpz_editor_open;
        egui::Window::new("Редактор KPZ")
            .open(&mut open)
            .resizable(true)
            .default_size([760.0, 560.0])
            .show(ctx, |ui| {
                ui.label(format!("KPZ: {}", self.selected_kpz_name()));
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("ID:");
                    ui.add(egui::TextEdit::singleline(&mut self.kpz_editor_id).desired_width(90.0));
                    ui.label("Имя:");
                    ui.add(egui::TextEdit::singleline(&mut self.kpz_editor_name).desired_width(280.0));
                    ui.label("RTU:");
                    ui.add(egui::TextEdit::singleline(&mut self.kpz_editor_rtu).desired_width(90.0));
                    ui.label("Модем:");
                    ui.add(egui::TextEdit::singleline(&mut self.kpz_editor_modem).desired_width(90.0));
                });

                ui.horizontal(|ui| {
                    ui.label("OBJ:");
                    let mut selected_obj = self.kpz_editor_obj;
                    egui::ComboBox::from_id_salt("kpz_editor_obj")
                        .selected_text(self.obj_caption(selected_obj))
                        .show_ui(ui, |ui| {
                            for obj in &self.obj_rows {
                                let label = if obj.name.is_empty() {
                                    obj.id.to_string()
                                } else {
                                    format!("{} - {}", obj.id, obj.name)
                                };
                                ui.selectable_value(&mut selected_obj, Some(obj.id), label);
                            }
                        });
                    self.kpz_editor_obj = selected_obj;
                    ui.label("max_pkt_len:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.kpz_editor_max_pkt_len)
                            .desired_width(90.0),
                    );
                    ui.checkbox(&mut self.kpz_editor_start, "Старт");
                    ui.checkbox(&mut self.kpz_editor_en_post, "en_post");
                });

                ui.horizontal(|ui| {
                    ui.label("t_a:");
                    ui.add(egui::TextEdit::singleline(&mut self.kpz_editor_t_a).desired_width(90.0));
                    ui.label("t_script:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.kpz_editor_t_script)
                            .desired_width(90.0),
                    );
                });

                let obj = self
                    .kpz_editor_obj
                    .and_then(|id| self.obj_rows.iter().find(|o| o.id == id));
                ui.separator();
                ui.heading("Расшифровка OBJ");
                if let Some(obj) = obj {
                    let obj_name = if obj.name.is_empty() { "-" } else { obj.name.as_str() };
                    ui.label(format!("obj: {} - {}", obj.id, obj_name));
                    ui.label(format!("ip: {}", Self::ref_caption(&self.ref_ip, obj.ip)));
                    ui.label(format!("port: {}", Self::ref_caption(&self.ref_port, obj.port)));
                    ui.label(format!("kanal: {}", Self::ref_caption(&self.ref_kanal, obj.kanal)));
                    ui.label(format!("speed: {}", Self::ref_caption(&self.ref_speed, obj.speed)));
                    ui.label(format!("stop: {}", Self::ref_caption(&self.ref_stop, obj.stop)));
                    ui.label(format!("parit: {}", Self::ref_caption(&self.ref_parit, obj.parit)));
                    ui.label(format!("bit: {}", Self::ref_caption(&self.ref_bit, obj.bit)));
                } else {
                    ui.label("OBJ не выбран или не найден");
                }

                if let Some(err) = &self.kpz_editor_err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &self.kpz_editor_status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }

                let dirty = self
                    .selected_kpz
                    .and_then(|id| self.kpz.iter().find(|k| k.id == id))
                    .map(|k| {
                        self.kpz_editor_name != k.name
                            || self.kpz_editor_rtu != k.rtu.to_string()
                            || self.kpz_editor_obj != Some(k.obj)
                            || self.kpz_editor_modem != k.modem.map(|v| v.to_string()).unwrap_or_default()
                            || self.kpz_editor_max_pkt_len != k.max_pkt_len.unwrap_or(800).to_string()
                            || self.kpz_editor_start != (k.start == 1)
                            || self.kpz_editor_en_post != k.en_post
                            || self.kpz_editor_t_a != k.t_a
                            || self.kpz_editor_t_script != k.t_script
                    })
                    .unwrap_or(false);

                ui.horizontal(|ui| {
                    if ui.button("Новый").clicked() {
                        self.create_kpz_new();
                    }
                    if ui.add_enabled(dirty, egui::Button::new("Сохранить")).clicked() {
                        self.save_kpz_full();
                    }
                    if ui.button("Обновить").clicked() {
                        self.sync_kpz_full_editor_from_selected();
                    }
                    if dirty {
                        ui.label("Есть несохраненные изменения");
                    }
                });
            });
        self.kpz_editor_open = open;
    }
}

