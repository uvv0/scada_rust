use crate::app::Ss5App;
use eframe::egui;

impl Ss5App {
    pub(crate) fn open_obj_editor(&mut self) {
        if self.obj_rows.is_empty() {
            self.reload_kpz_refs();
        }
        self.obj_editor_selected_id = self
            .selected_kpz
            .and_then(|id| self.kpz.iter().find(|k| k.id == id))
            .map(|k| k.obj)
            .or_else(|| self.obj_rows.first().map(|o| o.id));
        self.sync_obj_editor_from_selected();
        self.obj_editor_open = true;
    }

    fn sync_obj_editor_from_selected(&mut self) {
        if let Some(row) = self
            .obj_editor_selected_id
            .and_then(|id| self.obj_rows.iter().find(|o| o.id == id))
        {
            self.obj_editor_name = row.name.clone();
            self.obj_editor_ip = row.ip;
            self.obj_editor_port = row.port;
            self.obj_editor_kanal = row.kanal;
            self.obj_editor_speed = row.speed;
            self.obj_editor_stop = row.stop;
            self.obj_editor_parit = row.parit;
            self.obj_editor_bit = row.bit;
        } else {
            self.obj_editor_name.clear();
            self.obj_editor_ip = None;
            self.obj_editor_port = None;
            self.obj_editor_kanal = None;
            self.obj_editor_speed = None;
            self.obj_editor_stop = None;
            self.obj_editor_parit = None;
            self.obj_editor_bit = None;
        }
        self.obj_editor_err = None;
        self.obj_editor_status = None;
    }

    fn save_obj_editor(&mut self) {
        let name = self.obj_editor_name.trim();
        if name.is_empty() {
            self.obj_editor_err = Some("Имя обязательно".to_string());
            return;
        }
        let saved_id = if let Some(id) = self.obj_editor_selected_id {
            if let Err(e) = self.db.update_obj(
                id,
                name,
                self.obj_editor_ip,
                self.obj_editor_port,
                self.obj_editor_kanal,
                self.obj_editor_speed,
                self.obj_editor_stop,
                self.obj_editor_parit,
                self.obj_editor_bit,
            ) {
                self.obj_editor_err = Some(format!("update_obj failed: {e}"));
                return;
            }
            if let Some(obj) = self.obj_rows.iter_mut().find(|o| o.id == id) {
                obj.name = name.to_string();
                obj.ip_raw = self.obj_editor_ip.map(|v| v.to_string());
                obj.ip = self.obj_editor_ip;
                obj.port = self.obj_editor_port;
                obj.kanal = self.obj_editor_kanal;
                obj.speed = self.obj_editor_speed;
                obj.stop = self.obj_editor_stop;
                obj.parit = self.obj_editor_parit;
                obj.bit = self.obj_editor_bit;
            }
            id
        } else {
            let id = match self.db.create_obj(
                name,
                self.obj_editor_ip,
                self.obj_editor_port,
                self.obj_editor_kanal,
                self.obj_editor_speed,
                self.obj_editor_stop,
                self.obj_editor_parit,
                self.obj_editor_bit,
            ) {
                Ok(id) => id,
                Err(e) => {
                    self.obj_editor_err = Some(format!("create_obj failed: {e}"));
                    return;
                }
            };
            self.obj_rows.push(crate::models::ObjRow {
                id,
                name: name.to_string(),
                ip_raw: self.obj_editor_ip.map(|v| v.to_string()),
                ip: self.obj_editor_ip,
                port: self.obj_editor_port,
                kanal: self.obj_editor_kanal,
                speed: self.obj_editor_speed,
                stop: self.obj_editor_stop,
                parit: self.obj_editor_parit,
                bit: self.obj_editor_bit,
            });
            self.obj_rows.sort_by_key(|o| o.id);
            self.obj_editor_selected_id = Some(id);
            id
        };
        self.obj_editor_err = None;
        self.obj_editor_status = Some("Сохранено".to_string());
        self.push_log(format!("OBJ {} сохранен", saved_id));
    }

    pub(crate) fn show_obj_editor(&mut self, ctx: &egui::Context) {
        if !self.obj_editor_open {
            return;
        }

        let mut open = self.obj_editor_open;
        egui::Window::new("Редактор OBJ")
            .open(&mut open)
            .resizable(true)
            .default_size([760.0, 460.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("OBJ:");
                    let mut selected = self.obj_editor_selected_id;
                    let selected_text = selected
                        .map(|id| self.obj_caption(Some(id)))
                        .unwrap_or_else(|| "<нет>".to_string());
                    egui::ComboBox::from_id_salt("obj_editor_combo")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            for obj in &self.obj_rows {
                                let label = if obj.name.is_empty() {
                                    obj.id.to_string()
                                } else {
                                    format!("{} - {}", obj.id, obj.name)
                                };
                                ui.selectable_value(&mut selected, Some(obj.id), label);
                            }
                        });
                    if selected != self.obj_editor_selected_id {
                        self.obj_editor_selected_id = selected;
                        self.sync_obj_editor_from_selected();
                    }
                    if ui.button("Обновить").clicked() {
                        self.reload_kpz_refs();
                        self.sync_obj_editor_from_selected();
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Имя:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.obj_editor_name).desired_width(280.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("ip:");
                    let mut ip = self.obj_editor_ip;
                    egui::ComboBox::from_id_salt("obj_editor_ip")
                        .selected_text(Self::ref_caption(&self.ref_ip, ip))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut ip, None, "-");
                            for (id, name) in &self.ref_ip {
                                ui.selectable_value(
                                    &mut ip,
                                    Some(*id),
                                    format!("{} - {}", id, name),
                                );
                            }
                        });
                    self.obj_editor_ip = ip;

                    ui.label("port:");
                    let mut port = self.obj_editor_port;
                    egui::ComboBox::from_id_salt("obj_editor_port")
                        .selected_text(Self::ref_caption(&self.ref_port, port))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut port, None, "-");
                            for (id, name) in &self.ref_port {
                                ui.selectable_value(
                                    &mut port,
                                    Some(*id),
                                    format!("{} - {}", id, name),
                                );
                            }
                        });
                    self.obj_editor_port = port;
                });
                ui.horizontal(|ui| {
                    ui.label("kanal:");
                    let mut kanal = self.obj_editor_kanal;
                    egui::ComboBox::from_id_salt("obj_editor_kanal")
                        .selected_text(Self::ref_caption(&self.ref_kanal, kanal))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut kanal, None, "-");
                            for (id, name) in &self.ref_kanal {
                                ui.selectable_value(
                                    &mut kanal,
                                    Some(*id),
                                    format!("{} - {}", id, name),
                                );
                            }
                        });
                    self.obj_editor_kanal = kanal;

                    ui.label("speed:");
                    let mut speed = self.obj_editor_speed;
                    egui::ComboBox::from_id_salt("obj_editor_speed")
                        .selected_text(Self::ref_caption(&self.ref_speed, speed))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut speed, None, "-");
                            for (id, name) in &self.ref_speed {
                                ui.selectable_value(
                                    &mut speed,
                                    Some(*id),
                                    format!("{} - {}", id, name),
                                );
                            }
                        });
                    self.obj_editor_speed = speed;
                });
                ui.horizontal(|ui| {
                    ui.label("stop:");
                    let mut stop = self.obj_editor_stop;
                    egui::ComboBox::from_id_salt("obj_editor_stop")
                        .selected_text(Self::ref_caption(&self.ref_stop, stop))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut stop, None, "-");
                            for (id, name) in &self.ref_stop {
                                ui.selectable_value(
                                    &mut stop,
                                    Some(*id),
                                    format!("{} - {}", id, name),
                                );
                            }
                        });
                    self.obj_editor_stop = stop;

                    ui.label("parit:");
                    let mut parit = self.obj_editor_parit;
                    egui::ComboBox::from_id_salt("obj_editor_parit")
                        .selected_text(Self::ref_caption(&self.ref_parit, parit))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut parit, None, "-");
                            for (id, name) in &self.ref_parit {
                                ui.selectable_value(
                                    &mut parit,
                                    Some(*id),
                                    format!("{} - {}", id, name),
                                );
                            }
                        });
                    self.obj_editor_parit = parit;

                    ui.label("bit:");
                    let mut bit = self.obj_editor_bit;
                    egui::ComboBox::from_id_salt("obj_editor_bit")
                        .selected_text(Self::ref_caption(&self.ref_bit, bit))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut bit, None, "-");
                            for (id, name) in &self.ref_bit {
                                ui.selectable_value(
                                    &mut bit,
                                    Some(*id),
                                    format!("{} - {}", id, name),
                                );
                            }
                        });
                    self.obj_editor_bit = bit;
                });

                if let Some(err) = &self.obj_editor_err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &self.obj_editor_status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }

                let dirty = self
                    .obj_editor_selected_id
                    .and_then(|id| self.obj_rows.iter().find(|o| o.id == id))
                    .map(|o| {
                        self.obj_editor_name != o.name
                            || self.obj_editor_ip != o.ip
                            || self.obj_editor_port != o.port
                            || self.obj_editor_kanal != o.kanal
                            || self.obj_editor_speed != o.speed
                            || self.obj_editor_stop != o.stop
                            || self.obj_editor_parit != o.parit
                            || self.obj_editor_bit != o.bit
                    })
                    .unwrap_or_else(|| {
                        !self.obj_editor_name.trim().is_empty()
                            || self.obj_editor_ip.is_some()
                            || self.obj_editor_port.is_some()
                            || self.obj_editor_kanal.is_some()
                            || self.obj_editor_speed.is_some()
                            || self.obj_editor_stop.is_some()
                            || self.obj_editor_parit.is_some()
                            || self.obj_editor_bit.is_some()
                    });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(dirty, egui::Button::new("Сохранить"))
                        .clicked()
                    {
                        self.save_obj_editor();
                    }
                    if ui.button("Новая").clicked() {
                        self.obj_editor_selected_id = None;
                        self.obj_editor_status = None;
                        self.obj_editor_err = None;
                    }
                    if dirty {
                        ui.label("Есть несохраненные изменения");
                    }
                });
            });
        self.obj_editor_open = open;
    }
}

