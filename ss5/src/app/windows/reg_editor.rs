use crate::app::Ss5App;
use eframe::egui;

impl Ss5App {
    fn reg_group_label(&self, group_id: i32) -> String {
        self.groups
            .iter()
            .find(|g| g.id == group_id)
            .map(|g| format!("{} - {}", g.id, g.name))
            .unwrap_or_else(|| group_id.to_string())
    }

    fn reg_dict_label(&self, map: &std::collections::HashMap<i32, String>, value: i32) -> String {
        map.get(&value)
            .map(|name| format!("{} - {}", value, name))
            .unwrap_or_else(|| value.to_string())
    }

    pub(crate) fn open_reg_editor(&mut self) {
        self.reload_reg_rows();
        self.reg_editor_open = true;
    }

    fn reload_reg_rows(&mut self) {
        match self.db.get_all_reg_edit() {
            Ok(v) => {
                self.reg_rows = v;
                if self.reg_editor_selected_id.is_none() {
                    self.reg_editor_selected_id = self.reg_rows.first().map(|r| r.id);
                } else if let Some(id) = self.reg_editor_selected_id {
                    if !self.reg_rows.iter().any(|r| r.id == id) {
                        self.reg_editor_selected_id = self.reg_rows.first().map(|r| r.id);
                    }
                }
                self.sync_reg_editor_from_selected();
            }
            Err(e) => self.reg_editor_err = Some(format!("get_all_reg_edit failed: {e}")),
        }
    }

    fn sync_reg_editor_from_selected(&mut self) {
        if let Some(row) = self
            .reg_editor_selected_id
            .and_then(|id| self.reg_rows.iter().find(|r| r.id == id))
        {
            self.reg_editor_id = row.id.to_string();
            self.reg_editor_name = row.name.clone();
            self.reg_editor_mb = row.mb.to_string();
            self.reg_editor_n_mb = row.n_mb.map(|v| v.to_string()).unwrap_or_default();
            self.reg_editor_tip = row.tip.to_string();
            self.reg_editor_bits = row.bits.map(|v| v.to_string()).unwrap_or_default();
            self.reg_editor_grup = row.grup.map(|v| v.to_string()).unwrap_or_default();
            self.reg_editor_a_en = row.a_en;
            self.reg_editor_a_no_write = row.a_no_write.to_string();
        } else {
            self.reg_editor_id.clear();
            self.reg_editor_name.clear();
            self.reg_editor_mb.clear();
            self.reg_editor_n_mb.clear();
            self.reg_editor_tip.clear();
            self.reg_editor_bits.clear();
            self.reg_editor_grup.clear();
            self.reg_editor_a_en = false;
            self.reg_editor_a_no_write.clear();
        }
        self.reg_editor_err = None;
        self.reg_editor_status = None;
    }

    fn save_reg_editor(&mut self) {
        let id = match self.reg_editor_id.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.reg_editor_err = Some("id должен быть целым числом".to_string());
                return;
            }
        };
        let name = self.reg_editor_name.trim();
        if name.is_empty() {
            self.reg_editor_err = Some("Имя обязательно".to_string());
            return;
        }
        let mb = match self.reg_editor_mb.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.reg_editor_err = Some("mb должен быть целым числом".to_string());
                return;
            }
        };
        let n_mb = {
            let s = self.reg_editor_n_mb.trim();
            if s.is_empty() {
                None
            } else {
                match s.parse::<i32>() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        self.reg_editor_err = Some("n_mb должен быть целым числом или пустым".to_string());
                        return;
                    }
                }
            }
        };
        let tip = match self.reg_editor_tip.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.reg_editor_err = Some("tip должен быть целым числом".to_string());
                return;
            }
        };
        let bits = {
            let s = self.reg_editor_bits.trim();
            if s.is_empty() {
                None
            } else {
                match s.parse::<i32>() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        self.reg_editor_err = Some("bits должен быть целым числом или пустым".to_string());
                        return;
                    }
                }
            }
        };
        let grup = match self.reg_editor_grup.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.reg_editor_err = Some("grup должен быть целым числом".to_string());
                return;
            }
        };
        let a_no_write = match self.reg_editor_a_no_write.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.reg_editor_err = Some("a_no_write должен быть целым числом".to_string());
                return;
            }
        };
        if a_no_write != 0 && a_no_write != 1 {
            self.reg_editor_err = Some("a_no_write должен быть 0 или 1".to_string());
            return;
        }
        if let Err(e) = self.db.update_reg_edit(
            id,
            name,
            mb,
            n_mb,
            tip,
            bits,
            grup,
            self.reg_editor_a_en,
            a_no_write,
        ) {
            self.reg_editor_err = Some(format!("update_reg_edit failed: {e}"));
            return;
        }
        self.reg_editor_selected_id = Some(id);
        self.reload_reg_rows();
        self.reg_editor_selected_id = Some(id);
        self.sync_reg_editor_from_selected();
        self.reg_editor_err = None;
        self.reg_editor_status = Some("Сохранено".to_string());
        self.push_log(format!("REG {} сохранен", id));
    }

    pub(crate) fn show_reg_editor(&mut self, ctx: &egui::Context) {
        if !self.reg_editor_open {
            return;
        }

        let mut open = self.reg_editor_open;
        egui::Window::new("Редактор REG")
            .open(&mut open)
            .resizable(true)
            .default_size([900.0, 560.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Группа:");
                    let mut grp = self.reg_editor_group_filter;
                    let mut groups: Vec<i32> = self.reg_rows.iter().filter_map(|r| r.grup).collect();
                    groups.sort_unstable();
                    groups.dedup();
                    egui::ComboBox::from_id_salt("reg_editor_group_filter")
                        .selected_text(
                            grp.map(|v| self.reg_group_label(v))
                                .unwrap_or_else(|| "все".to_string()),
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut grp, None, "все");
                            for group_id in &groups {
                                ui.selectable_value(
                                    &mut grp,
                                    Some(*group_id),
                                    self.reg_group_label(*group_id),
                                );
                            }
                        });
                    self.reg_editor_group_filter = grp;

                    ui.label("Фильтр:");
                    ui.text_edit_singleline(&mut self.reg_editor_filter);

                    if ui.button("Обновить").clicked() {
                        self.open_reg_editor();
                    }
                });

                egui::SidePanel::left("reg_editor_list")
                    .resizable(true)
                    .default_width(320.0)
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let filter = self.reg_editor_filter.trim().to_lowercase();
                            let mut clicked_id: Option<i32> = None;
                            for r in &self.reg_rows {
                                if self.reg_editor_group_filter.is_some()
                                    && r.grup != self.reg_editor_group_filter
                                {
                                    continue;
                                }
                                if !filter.is_empty() {
                                    let hay =
                                        format!("{} {} {}", r.id, r.name.to_lowercase(), r.mb);
                                    if !hay.contains(&filter) {
                                        continue;
                                    }
                                }
                                let label = format!("{} | mb={} | {}", r.id, r.mb, r.name);
                                if ui
                                    .selectable_label(self.reg_editor_selected_id == Some(r.id), label)
                                    .clicked()
                                {
                                    clicked_id = Some(r.id);
                                }
                            }
                            if let Some(id) = clicked_id {
                                self.reg_editor_selected_id = Some(id);
                                self.sync_reg_editor_from_selected();
                            }
                        });
                    });

                ui.separator();
                ui.heading("Запись");
                ui.horizontal(|ui| {
                    ui.label("id:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.reg_editor_id).desired_width(90.0),
                    );
                    ui.label("name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.reg_editor_name).desired_width(320.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("mb:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.reg_editor_mb).desired_width(80.0),
                    );
                    ui.label("n_mb:");
                    let mut n_mb_value = self.reg_editor_n_mb.trim().parse::<i32>().ok();
                    egui::ComboBox::from_id_salt("reg_editor_n_mb")
                        .selected_text(
                            n_mb_value
                                .map(|v| self.reg_dict_label(&self.ref_n_mb, v))
                                .unwrap_or_else(|| {
                                    if self.reg_editor_n_mb.trim().is_empty() {
                                        "-".to_string()
                                    } else {
                                        self.reg_editor_n_mb.clone()
                                    }
                                }),
                        )
                        .width(170.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut n_mb_value, None, "-");
                            let mut items: Vec<(i32, String)> =
                                self.ref_n_mb.iter().map(|(id, name)| (*id, name.clone())).collect();
                            items.sort_by_key(|(id, _)| *id);
                            for (id, _) in items {
                                ui.selectable_value(
                                    &mut n_mb_value,
                                    Some(id),
                                    self.reg_dict_label(&self.ref_n_mb, id),
                                );
                            }
                        });
                    self.reg_editor_n_mb = n_mb_value.map(|v| v.to_string()).unwrap_or_default();
                    ui.label("tip:");
                    let mut tip_value = self.reg_editor_tip.trim().parse::<i32>().ok().unwrap_or_default();
                    egui::ComboBox::from_id_salt("reg_editor_tip")
                        .selected_text(self.reg_dict_label(&self.ref_tip, tip_value))
                        .width(170.0)
                        .show_ui(ui, |ui| {
                            let mut items: Vec<(i32, String)> =
                                self.ref_tip.iter().map(|(id, name)| (*id, name.clone())).collect();
                            items.sort_by_key(|(id, _)| *id);
                            for (id, _) in items {
                                ui.selectable_value(
                                    &mut tip_value,
                                    id,
                                    self.reg_dict_label(&self.ref_tip, id),
                                );
                            }
                        });
                    self.reg_editor_tip = tip_value.to_string();
                    ui.label("bits:");
                    let mut bits_value = self.reg_editor_bits.trim().parse::<i32>().ok();
                    egui::ComboBox::from_id_salt("reg_editor_bits")
                        .selected_text(
                            bits_value
                                .map(|v| self.reg_dict_label(&self.ref_c, v))
                                .unwrap_or_else(|| {
                                    if self.reg_editor_bits.trim().is_empty() {
                                        "-".to_string()
                                    } else {
                                        self.reg_editor_bits.clone()
                                    }
                                }),
                        )
                        .width(170.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut bits_value, None, "-");
                            let mut items: Vec<(i32, String)> =
                                self.ref_c.iter().map(|(id, name)| (*id, name.clone())).collect();
                            items.sort_by_key(|(id, _)| *id);
                            for (id, _) in items {
                                ui.selectable_value(
                                    &mut bits_value,
                                    Some(id),
                                    self.reg_dict_label(&self.ref_c, id),
                                );
                            }
                        });
                    self.reg_editor_bits = bits_value.map(|v| v.to_string()).unwrap_or_default();
                });
                ui.horizontal(|ui| {
                    ui.label("grup:");
                    let mut grup_value = self.reg_editor_grup.trim().parse::<i32>().ok();
                    egui::ComboBox::from_id_salt("reg_editor_grup")
                        .selected_text(
                            grup_value
                                .map(|v| self.reg_group_label(v))
                                .unwrap_or_else(|| {
                                    if self.reg_editor_grup.trim().is_empty() {
                                        "-".to_string()
                                    } else {
                                        self.reg_editor_grup.clone()
                                    }
                                }),
                        )
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for g in &self.groups {
                                ui.selectable_value(
                                    &mut grup_value,
                                    Some(g.id),
                                    self.reg_group_label(g.id),
                                );
                            }
                        });
                    if let Some(group_id) = grup_value {
                        self.reg_editor_grup = group_id.to_string();
                    }
                    ui.checkbox(&mut self.reg_editor_a_en, "a_en");
                    ui.label("a_no_write:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.reg_editor_a_no_write)
                            .desired_width(90.0),
                    );
                });

                if let Some(err) = &self.reg_editor_err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &self.reg_editor_status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }

                let dirty = self
                    .reg_editor_selected_id
                    .and_then(|id| self.reg_rows.iter().find(|r| r.id == id))
                    .map(|r| {
                        self.reg_editor_id != r.id.to_string()
                            || self.reg_editor_name != r.name
                            || self.reg_editor_mb != r.mb.to_string()
                            || self.reg_editor_n_mb
                                != r.n_mb.map(|v| v.to_string()).unwrap_or_default()
                            || self.reg_editor_tip != r.tip.to_string()
                            || self.reg_editor_bits
                                != r.bits.map(|v| v.to_string()).unwrap_or_default()
                            || self.reg_editor_grup
                                != r.grup.map(|v| v.to_string()).unwrap_or_default()
                            || self.reg_editor_a_en != r.a_en
                            || self.reg_editor_a_no_write != r.a_no_write.to_string()
                    })
                    .unwrap_or_else(|| {
                        !self.reg_editor_id.trim().is_empty()
                            || !self.reg_editor_name.trim().is_empty()
                            || !self.reg_editor_mb.trim().is_empty()
                            || !self.reg_editor_n_mb.trim().is_empty()
                            || !self.reg_editor_tip.trim().is_empty()
                            || !self.reg_editor_bits.trim().is_empty()
                            || !self.reg_editor_grup.trim().is_empty()
                            || self.reg_editor_a_en
                            || !self.reg_editor_a_no_write.trim().is_empty()
                    });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(dirty, egui::Button::new("Сохранить"))
                        .clicked()
                    {
                        self.save_reg_editor();
                    }
                    if ui.button("Новая").clicked() {
                        self.reg_editor_selected_id = None;
                        self.reg_editor_status = None;
                        self.reg_editor_err = None;
                    }
                    if dirty {
                        ui.label("Есть несохраненные изменения");
                    }
                });
            });
        self.reg_editor_open = open;
    }
}

