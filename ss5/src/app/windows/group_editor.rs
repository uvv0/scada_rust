use crate::app::Ss5App;
use eframe::egui;

impl Ss5App {
    pub(crate) fn open_group_editor(&mut self) {
        self.group_edit_selected = self.selected_enabled_groups().into_iter().collect();
        self.group_edit_err = None;
        self.group_edit_dirty = false;
        self.group_editor_open = true;
    }

    fn save_groups(&mut self) {
        let Some(id) = self.selected_kpz else {
            self.group_edit_err = Some("KPZ не выбран".to_string());
            return;
        };
        for &g in &self.group_edit_selected {
            if g < 1 || g > 512 {
                self.group_edit_err = Some(format!("Идентификатор группы вне диапазона: {}", g));
                return;
            }
        }
        let grups = crate::utils::encode_groups(&self.group_edit_selected);
        if let Err(e) = self.db.update_kpz_grups(id, &grups) {
            self.group_edit_err = Some(format!("update_kpz_grups failed: {e}"));
            return;
        }
        for row in &mut self.kpz {
            if row.id == id {
                row.grups = grups.clone();
                break;
            }
        }
        self.group_edit_dirty = false;
        self.group_edit_err = None;
    }

    pub(crate) fn show_group_editor(&mut self, ctx: &egui::Context) {
        if !self.group_editor_open {
            return;
        }

        let mut open = self.group_editor_open;
        egui::Window::new("Группы")
            .open(&mut open)
            .resizable(true)
            .default_size([560.0, 520.0])
            .show(ctx, |ui| {
                ui.label(format!("KPZ: {}", self.selected_kpz_name()));
                ui.horizontal(|ui| {
                    ui.label("Фильтр:");
                    ui.add(egui::TextEdit::singleline(&mut self.group_edit_filter).desired_width(180.0));
                    if ui.button("Все").clicked() {
                        self.group_edit_selected = self.groups.iter().map(|g| g.id).collect();
                        self.group_edit_dirty = true;
                    }
                    if ui.button("Снять все").clicked() {
                        self.group_edit_selected.clear();
                        self.group_edit_dirty = true;
                    }
                });
                ui.separator();
                let filter = self.group_edit_filter.trim().to_lowercase();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for g in &self.groups {
                        let label = if g.name.is_empty() {
                            g.id.to_string()
                        } else {
                            format!("{} - {}", g.id, g.name)
                        };
                        if !filter.is_empty() && !label.to_lowercase().contains(&filter) {
                            continue;
                        }
                        let mut checked = self.group_edit_selected.contains(&g.id);
                        if ui.checkbox(&mut checked, label).changed() {
                            if checked {
                                self.group_edit_selected.insert(g.id);
                            } else {
                                self.group_edit_selected.remove(&g.id);
                            }
                            self.group_edit_dirty = true;
                        }
                    }
                });
                ui.separator();
                if let Some(err) = &self.group_edit_err {
                    ui.colored_label(egui::Color32::RED, err);
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.group_edit_dirty, egui::Button::new("Сохранить")).clicked() {
                        self.save_groups();
                    }
                    if self.group_edit_dirty {
                        ui.label("Есть несохраненные изменения");
                    }
                });
            });
        self.group_editor_open = open;
    }
}
