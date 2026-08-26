use crate::app::Ss5App;
use eframe::egui;

impl Ss5App {
    pub(crate) fn open_range_kpz_window(&mut self) {
        if self.obj_rows.is_empty() {
            self.reload_kpz_refs();
        }
        if self.range_kpz_obj.is_none() {
            self.range_kpz_obj = self
                .obj_rows
                .iter()
                .find(|o| o.id == 5)
                .map(|o| o.id)
                .or_else(|| self.obj_rows.first().map(|o| o.id));
        }
        self.range_kpz_err = None;
        self.range_kpz_status = None;
        if self.range_kpz_groups_selected.is_empty() {
            self.range_kpz_groups_selected = self.selected_enabled_groups().into_iter().collect();
        }
        self.range_kpz_open = true;
    }

    fn parse_range_kpz_params(&self) -> Result<(i32, i32, i32, i32), String> {
        let id_start = self.range_kpz_id_start.trim().parse::<i32>().map_err(|_| "начало диапазона должно быть целым числом".to_string())?;
        let id_end = self.range_kpz_id_end.trim().parse::<i32>().map_err(|_| "конец диапазона должен быть целым числом".to_string())?;
        if id_end < id_start {
            return Err("конец диапазона должен быть >= начала".to_string());
        }
        if (id_end - id_start + 1) > 10_000 {
            return Err("диапазон слишком большой (>10000)".to_string());
        }
        let obj_id = self.range_kpz_obj.ok_or_else(|| "OBJ должен быть выбран".to_string())?;
        let modem_start = self.range_kpz_modem_start.trim().parse::<i32>().map_err(|_| "начальный modem должен быть целым числом".to_string())?;
        Ok((id_start, id_end, obj_id, modem_start))
    }

    fn parse_range_kpz_ids(&self) -> Result<(i32, i32), String> {
        let id_start = self.range_kpz_id_start.trim().parse::<i32>().map_err(|_| "начало диапазона должно быть целым числом".to_string())?;
        let id_end = self.range_kpz_id_end.trim().parse::<i32>().map_err(|_| "конец диапазона должен быть целым числом".to_string())?;
        if id_end < id_start {
            return Err("конец диапазона должен быть >= начала".to_string());
        }
        if (id_end - id_start + 1) > 10_000 {
            return Err("диапазон слишком большой (>10000)".to_string());
        }
        Ok((id_start, id_end))
    }

    fn parse_range_kpz_timing(&self) -> Result<(Option<i32>, Option<i32>), String> {
        let t_a = {
            let s = self.range_kpz_t_a.trim();
            if s.is_empty() { None } else { Some(s.parse::<i32>().map_err(|_| "t_a должен быть целым числом или пустым".to_string())?) }
        };
        let t_script = {
            let s = self.range_kpz_t_script.trim();
            if s.is_empty() { None } else { Some(s.parse::<i32>().map_err(|_| "t_script должен быть целым числом или пустым".to_string())?) }
        };
        Ok((t_a, t_script))
    }

    fn create_or_update_range_kpz(&mut self) {
        let (id_start, id_end, obj_id, modem_start) = match self.parse_range_kpz_params() {
            Ok(v) => v,
            Err(e) => {
                self.range_kpz_err = Some(e);
                return;
            }
        };
        if !self.obj_rows.iter().any(|o| o.id == obj_id) {
            self.range_kpz_err = Some(format!("obj {} не найден в таблице obj", obj_id));
            return;
        }
        let max_pkt_len = 800;
        let n = match self.db.upsert_test_kpz_range(id_start, id_end, obj_id, modem_start, max_pkt_len) {
            Ok(v) => v,
            Err(e) => {
                self.range_kpz_err = Some(format!("upsert_test_kpz_range failed: {e:?}"));
                return;
            }
        };
        self.reload_all();
        self.range_kpz_err = None;
        self.range_kpz_status = Some(format!("Диапазон {}..{} сохранен (затронуто строк: {})", id_start, id_end, n));
        self.push_log(format!("Range KPZ saved: {}..{}, obj={}, modem_start={}", id_start, id_end, obj_id, modem_start));
    }

    fn apply_range_kpz_start(&mut self, enabled: bool) {
        let (id_start, id_end) = match self.parse_range_kpz_ids() {
            Ok(v) => v,
            Err(e) => {
                self.range_kpz_err = Some(e);
                return;
            }
        };
        self.range_kpz_start_enabled = enabled;
        let n = match self.db.set_kpz_start_range(id_start, id_end, enabled) {
            Ok(v) => v,
            Err(e) => {
                self.range_kpz_err = Some(format!("set_kpz_start_range failed: {e}"));
                return;
            }
        };
        self.reload_all();
        self.range_kpz_err = None;
        self.range_kpz_status = Some(format!("start={} applied for {}..{} (rows: {})", if enabled { 1 } else { 0 }, id_start, id_end, n));
        if enabled {
            self.push_log(format!("Range KPZ start ON: {}..{}", id_start, id_end));
        } else {
            self.push_log(format!("Range KPZ start OFF: {}..{}", id_start, id_end));
        }
    }

    fn apply_range_kpz_timing(&mut self) {
        let (id_start, id_end) = match self.parse_range_kpz_ids() {
            Ok(v) => v,
            Err(e) => {
                self.range_kpz_err = Some(e);
                return;
            }
        };
        let (t_a, t_script) = match self.parse_range_kpz_timing() {
            Ok(v) => v,
            Err(e) => {
                self.range_kpz_err = Some(e);
                return;
            }
        };
        let n = match self.db.set_kpz_timing_range(id_start, id_end, t_a, t_script) {
            Ok(v) => v,
            Err(e) => {
                self.range_kpz_err = Some(format!("set_kpz_timing_range failed: {e}"));
                return;
            }
        };
        self.reload_all();
        self.range_kpz_err = None;
        self.range_kpz_status = Some(format!("t_a/t_script applied for {}..{} (rows: {})", id_start, id_end, n));
        self.push_log(format!("Range KPZ t_a/t_script applied: {}..{}, t_a={:?}, t_script={:?}", id_start, id_end, t_a, t_script));
    }

    fn apply_range_kpz_groups_enable(&mut self) {
        let (id_start, id_end) = match self.parse_range_kpz_ids() {
            Ok(v) => v,
            Err(e) => {
                self.range_kpz_err = Some(e);
                return;
            }
        };
        let add_groups = self.range_kpz_groups_selected.clone();
        if add_groups.is_empty() {
            self.range_kpz_err = Some("Не выбраны группы".to_string());
            return;
        }
        let mut matched = 0usize;
        let mut updates: Vec<(i32, Vec<u8>)> = Vec::new();
        for row in &self.kpz {
            if row.id < id_start || row.id > id_end {
                continue;
            }
            matched += 1;
            let mut groups: std::collections::BTreeSet<i32> = crate::utils::decode_groups(&row.grups).into_iter().collect();
            let before = groups.len();
            groups.extend(add_groups.iter().copied());
            if groups.len() != before {
                updates.push((row.id, crate::utils::encode_groups(&groups)));
            }
        }
        for (id, grups) in &updates {
            if let Err(e) = self.db.update_kpz_grups(*id, grups) {
                self.range_kpz_err = Some(format!("update_kpz_grups failed for kpz {}: {e}", id));
                return;
            }
        }
        for (id, grups) in &updates {
            if let Some(row) = self.kpz.iter_mut().find(|r| r.id == *id) {
                row.grups = grups.clone();
            }
        }
        self.range_kpz_err = None;
        self.range_kpz_status = Some(format!("groups enabled for {}..{} (matched: {}, changed: {})", id_start, id_end, matched, updates.len()));
        self.push_log(format!("Range KPZ groups enabled: {}..{}, groups={}", id_start, id_end, add_groups.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(",")));
    }

    fn apply_range_kpz_groups_disable(&mut self) {
        let (id_start, id_end) = match self.parse_range_kpz_ids() {
            Ok(v) => v,
            Err(e) => {
                self.range_kpz_err = Some(e);
                return;
            }
        };
        let remove_groups = self.range_kpz_groups_selected.clone();
        if remove_groups.is_empty() {
            self.range_kpz_err = Some("Не выбраны группы".to_string());
            return;
        }
        let mut matched = 0usize;
        let mut updates: Vec<(i32, Vec<u8>)> = Vec::new();
        for row in &self.kpz {
            if row.id < id_start || row.id > id_end {
                continue;
            }
            matched += 1;
            let mut groups: std::collections::BTreeSet<i32> = crate::utils::decode_groups(&row.grups).into_iter().collect();
            let before = groups.len();
            for g in &remove_groups {
                groups.remove(g);
            }
            if groups.len() != before {
                updates.push((row.id, crate::utils::encode_groups(&groups)));
            }
        }
        for (id, grups) in &updates {
            if let Err(e) = self.db.update_kpz_grups(*id, grups) {
                self.range_kpz_err = Some(format!("update_kpz_grups failed for kpz {}: {e}", id));
                return;
            }
        }
        for (id, grups) in &updates {
            if let Some(row) = self.kpz.iter_mut().find(|r| r.id == *id) {
                row.grups = grups.clone();
            }
        }
        self.range_kpz_err = None;
        self.range_kpz_status = Some(format!("groups disabled for {}..{} (matched: {}, changed: {})", id_start, id_end, matched, updates.len()));
        self.push_log(format!("Range KPZ groups disabled: {}..{}, groups={}", id_start, id_end, remove_groups.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(",")));
    }

    pub(crate) fn show_range_kpz_window(&mut self, ctx: &egui::Context) {
        if !self.range_kpz_open {
            return;
        }

        let mut open = self.range_kpz_open;
        egui::Window::new("Диапазон KPZ")
            .open(&mut open)
            .resizable(true)
            .default_size([860.0, 420.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Создание или обновление тестового диапазона KPZ и управление флагом start.");
                    if ui.small_button("?").clicked() {
                        self.range_kpz_help_open = !self.range_kpz_help_open;
                    }
                });
                if self.range_kpz_help_open {
                    ui.label("Сохранить диапазон: создать или обновить строки KPZ в диапазоне ID с выбранным OBJ и начальным modem.");
                    ui.label("Применить t_a/t_script: обновить t_a и t_script для диапазона ID. Пустое значение означает NULL.");
                    ui.label("Включить группы / Выключить группы: изменить выбранные группы для всего диапазона ID.");
                    ui.label("Запустить диапазон / Остановить диапазон: выставить start=1 или start=0 только для этого диапазона ID.");
                    ui.separator();
                }
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("ID range:");
                    ui.add(egui::TextEdit::singleline(&mut self.range_kpz_id_start).desired_width(90.0));
                    ui.label("..");
                    ui.add(egui::TextEdit::singleline(&mut self.range_kpz_id_end).desired_width(90.0));
                });

                ui.horizontal(|ui| {
                    ui.label("OBJ:");
                    let mut selected_obj = self.range_kpz_obj;
                    egui::ComboBox::from_id_salt("range_kpz_obj")
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
                    self.range_kpz_obj = selected_obj;
                });

                ui.horizontal(|ui| {
                    ui.label("Начальный modem:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.range_kpz_modem_start)
                            .desired_width(120.0),
                    );
                    ui.label("(пример: 5200)");
                });

                ui.horizontal(|ui| {
                    ui.label("t_a:");
                    ui.add(egui::TextEdit::singleline(&mut self.range_kpz_t_a).desired_width(90.0));
                    ui.label("t_script:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.range_kpz_t_script)
                            .desired_width(90.0),
                    );
                    ui.label("(пусто = NULL)");
                });

                ui.separator();
                ui.columns(2, |cols| {
                    cols[0].label(format!("Выбрано групп: {}", self.range_kpz_groups_selected.len()));
                    cols[0].horizontal(|ui| {
                        if ui.small_button("Все").clicked() {
                            self.range_kpz_groups_selected = self.groups.iter().map(|g| g.id).collect();
                        }
                        if ui.small_button("Снять все").clicked() {
                            self.range_kpz_groups_selected.clear();
                        }
                    });
                    cols[0].label("Выберите группы справа.");

                    cols[1].label("Группы:");
                    egui::ScrollArea::vertical()
                        .id_salt("range_kpz_groups_list")
                        .max_height(180.0)
                        .show(&mut cols[1], |ui| {
                            for g in self.groups.clone() {
                                let mut checked = self.range_kpz_groups_selected.contains(&g.id);
                                let label = if g.name.is_empty() {
                                    format!("{}", g.id)
                                } else {
                                    format!("{} - {}", g.id, g.name)
                                };
                                if ui.checkbox(&mut checked, label).changed() {
                                    if checked {
                                        self.range_kpz_groups_selected.insert(g.id);
                                    } else {
                                        self.range_kpz_groups_selected.remove(&g.id);
                                    }
                                }
                            }
                        });
                });

                ui.horizontal(|ui| {
                    if ui.button("Сохранить диапазон").clicked() {
                        self.create_or_update_range_kpz();
                    }
                    if ui.button("Применить t_a/t_script").clicked() {
                        self.apply_range_kpz_timing();
                    }
                    if ui.button("Включить группы").clicked() {
                        self.apply_range_kpz_groups_enable();
                    }
                    if ui.button("Выключить группы").clicked() {
                        self.apply_range_kpz_groups_disable();
                    }
                    if ui.button("Запустить диапазон").clicked() {
                        self.apply_range_kpz_start(true);
                    }
                    if ui.button("Остановить диапазон").clicked() {
                        self.apply_range_kpz_start(false);
                    }
                });

                if let Some(err) = &self.range_kpz_err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &self.range_kpz_status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }

                ui.separator();
                ui.label("Рекомендуемые тестовые значения:");
                ui.label("диапазон: 1200..1299, obj: 5, начальный modem: 5200");
                ui.label("При создании диапазона rtu остается фиксированным: 301. Меняются только id и modem.");
                ui.label("Запуск/остановка влияют только на диапазон ID и не трогают OBJ/Modem.");
            });
        self.range_kpz_open = open;
    }
}
