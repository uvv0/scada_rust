use crate::app::Ss5App;
use eframe::egui;

impl Ss5App {
    pub(crate) fn open_dict_editor(&mut self) {
        self.reload_dict_items();
        self.dict_editor_open = true;
    }

    fn reload_dict_items(&mut self) {
        match self.db.get_items(&self.dict_table) {
            Ok(v) => {
                self.dict_items = v;
                if self.dict_editor_selected_id.is_none() {
                    self.dict_editor_selected_id = self.dict_items.first().map(|d| d.id);
                } else if let Some(id) = self.dict_editor_selected_id {
                    if !self.dict_items.iter().any(|d| d.id == id) {
                        self.dict_editor_selected_id = self.dict_items.first().map(|d| d.id);
                    }
                }
                self.sync_dict_editor_from_selected();
            }
            Err(e) => self.dict_editor_err = Some(format!("get_items({}) failed: {e}", self.dict_table)),
        }
    }

    fn sync_dict_editor_from_selected(&mut self) {
        if let Some(row) = self
            .dict_editor_selected_id
            .and_then(|id| self.dict_items.iter().find(|d| d.id == id))
        {
            self.dict_editor_id = row.id.to_string();
            self.dict_editor_name = row.name.clone();
        } else {
            self.dict_editor_id.clear();
            self.dict_editor_name.clear();
        }
        self.dict_editor_err = None;
        self.dict_editor_status = None;
    }

    fn save_dict_editor(&mut self) {
        let id = match self.dict_editor_id.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.dict_editor_err = Some("id должен быть целым числом".to_string());
                return;
            }
        };
        let name = self.dict_editor_name.trim();
        if name.is_empty() {
            self.dict_editor_err = Some("Имя обязательно".to_string());
            return;
        }
        if let Err(e) = self.db.upsert_item(&self.dict_table, id, name) {
            self.dict_editor_err = Some(format!("upsert_item failed: {e}"));
            return;
        }
        self.reload_dict_items();
        self.reload_kpz_refs();
        self.dict_editor_selected_id = Some(id);
        self.sync_dict_editor_from_selected();
        self.dict_editor_status = Some("Сохранено".to_string());
        self.push_log(format!("Справочник {}:{} сохранен", self.dict_table, id));
    }

    fn delete_dict_editor(&mut self) {
        let id = match self.dict_editor_id.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.dict_editor_err = Some("id должен быть целым числом".to_string());
                return;
            }
        };
        if let Err(e) = self.db.delete_item(&self.dict_table, id) {
            self.dict_editor_err = Some(format!("delete_item failed: {e}"));
            return;
        }
        self.reload_dict_items();
        self.reload_kpz_refs();
        self.dict_editor_status = Some("Удалено".to_string());
        self.dict_editor_err = None;
        self.push_log(format!("Справочник {}:{} удален", self.dict_table, id));
    }

    pub(crate) fn show_dict_editor(&mut self, ctx: &egui::Context) {
        if !self.dict_editor_open {
            return;
        }

        let mut open = self.dict_editor_open;
        egui::Window::new("Редактор справочников")
            .open(&mut open)
            .resizable(true)
            .default_size([720.0, 520.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Таблица:");
                    let mut table = self.dict_table.clone();
                    egui::ComboBox::from_id_salt("dict_table")
                        .selected_text(table.clone())
                        .show_ui(ui, |ui| {
                            for t in [
                                "ip", "port", "speed", "parit", "bit", "stop", "kanal", "grup",
                                "n_mb", "tip", "bits", "c",
                            ] {
                                ui.selectable_value(&mut table, t.to_string(), t);
                            }
                        });
                    if table != self.dict_table {
                        self.dict_table = table;
                        self.reload_dict_items();
                    }
                    if ui.button("Обновить").clicked() {
                        self.reload_dict_items();
                    }
                });
                ui.separator();

                egui::SidePanel::left("dict_editor_list")
                    .resizable(true)
                    .default_width(300.0)
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let mut clicked_id: Option<i32> = None;
                            for d in &self.dict_items {
                                let label = format!("{} - {}", d.id, d.name);
                                if ui
                                    .selectable_label(self.dict_editor_selected_id == Some(d.id), label)
                                    .clicked()
                                {
                                    clicked_id = Some(d.id);
                                }
                            }
                            if let Some(id) = clicked_id {
                                self.dict_editor_selected_id = Some(id);
                                self.sync_dict_editor_from_selected();
                            }
                        });
                    });

                ui.heading("Запись");
                ui.horizontal(|ui| {
                    ui.label("id:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.dict_editor_id).desired_width(90.0),
                    );
                    ui.label("name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.dict_editor_name).desired_width(260.0),
                    );
                });
                if let Some(err) = &self.dict_editor_err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &self.dict_editor_status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }
                ui.horizontal(|ui| {
                    if ui.button("Сохранить").clicked() {
                        self.save_dict_editor();
                    }
                    if ui.button("Удалить").clicked() {
                        self.delete_dict_editor();
                    }
                    if ui.button("Новая").clicked() {
                        self.dict_editor_selected_id = None;
                        self.dict_editor_id.clear();
                        self.dict_editor_name.clear();
                        self.dict_editor_status = None;
                        self.dict_editor_err = None;
                    }
                });
            });
        self.dict_editor_open = open;
    }
}

