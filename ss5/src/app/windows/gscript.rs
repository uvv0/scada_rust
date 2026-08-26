use std::cell::RefCell;
use std::collections::HashMap;

use crate::app::{gscript_layout_job, GScriptOutputTab, GScriptTab, Ss5App};
use crate::models::{GScriptRow, GScriptTemplateRow};
use crate::script::Script;
use eframe::egui;
use serde_json::Value;

impl Ss5App {
    pub(crate) fn show_gscript_windows(&mut self, ctx: &egui::Context) {
        if self.gscript_open {
            let mut open = self.gscript_open;
            egui::Window::new("Редактор GScript")
                .open(&mut open)
                .resizable(true)
                .default_size([1040.0, 760.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Редактор PRE/POST сценариев и шаблонов группы.");
                        if ui.small_button("?").clicked() {
                            self.gscript_help_open = !self.gscript_help_open;
                        }
                    });
                    if self.gscript_help_open {
                        egui::ScrollArea::vertical()
                            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                            .max_height(230.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                        ui.label("1) Группа / Загрузить / Сохранить:");
                        ui.label("   - Группа выбирает группу.");
                        ui.label("   - Загрузить загружает прямой g_script этой группы.");
                        ui.label("   - Сохранить сохраняет текущий текст PRE/POST и параметры как прямой g_script.");
                        ui.label("   - Загрузить эффективный загружает эффективный скрипт: direct, иначе шаблон группы.");
                        ui.label("2) Проверить / Разобрать / Запустить:");
                        ui.label("   - Проверить валидирует диапазоны max_words/max_k и парсинг PRE/POST.");
                        ui.label("   - Разобрать PRE/POST делает только синтаксическую проверку.");
                        ui.label("   - Запустить PRE/POST выполняет скрипт на тестовых JSON words/regs, вывод в окно результата.");
                        ui.label("3) Тестовые входы:");
                        ui.label("   - words: JSON-массив u16 слов.");
                        ui.label("   - regs: JSON-объект {\"reg_id\": value}.");
                        ui.label("   - Старшее слово первым: меняет порядок слов при 32-битном декодировании.");
                        ui.label("   - Если включено, 32-битные значения собираются в порядке hi/lo вместо lo/hi.");
                        ui.label("   - Подстановка из arx_val: берет недостающие значения регистров из arx_val выбранного KPZ.");
                        ui.label("   - Полезно, когда в тестовом JSON указаны не все регистры, а остальное нужно взять из БД.");
                        ui.label("4) Шаблоны:");
                        ui.label("   - Загрузить шаблон загружает шаблон в редактор.");
                        ui.label("   - Сохранить шаблон сохраняет или обновляет шаблон.");
                        ui.label("   - Удалить шаблон удаляет выбранный шаблон.");
                        ui.label("   - Привязать к группе привязывает выбранный шаблон к группе.");
                        ui.label("5) Режим PRE/POST:");
                        ui.label("   - Вкладки PRE/POST переключают редактируемый текст.");
                        ui.label("   - Есть несохраненные изменения показывает изменения direct g_script.");
                        ui.label("6) Синтаксис:");
                        ui.label("   - let x = expr; создает переменную, x = expr; меняет переменную.");
                        ui.label("   - reg(reg_id, value); записывает значение регистра в результат.");
                        ui.label("   - emit(ts, reg_id, value); добавляет событие в emits.");
                        ui.label("   - if/else, while, for, блоки { ... } поддерживаются.");
                        ui.label("7) Функции:");
                        ui.label("   - u16/i16/u32/i32/f32(index) читают слова Modbus из words.");
                        ui.label("   - rv(reg_id) читает тестовый регистр или fallback из arx_val.");
                        ui.label("   - av(arx_id, reg_id) возвращает архивное значение; в тестовом запуске сейчас 0.");
                        ui.label("   - dt2unix(value) преобразует дату/время из формата скрипта в Unix.");
                        ui.label("   - bit(value, n) возвращает бит n.");
                        ui.label("   - abs/sqrt/floor/ceil/round/min/max/pow/clamp - математические функции.");
                        ui.label("   - print(value) и print2(a, b) пишут строки в окно вывода.");
                            });
                        ui.separator();
                    }

                    ui.horizontal(|ui| {
                        ui.label("Группа:");
                        let mut selected = self.gscript_group_id;
                        let selected_text = match selected {
                            Some(id) => {
                                if let Some(g) = self.groups.iter().find(|g| g.id == id) {
                                    if g.name.is_empty() { g.id.to_string() } else { format!("{} - {}", g.id, g.name) }
                                } else {
                                    format!("{} [g_script]", id)
                                }
                            }
                            None => "<нет>".to_string(),
                        };
                        egui::ComboBox::from_id_salt("gscript_group")
                            .selected_text(selected_text)
                            .show_ui(ui, |ui| {
                                for g in &self.groups {
                                    let label = if g.name.is_empty() { g.id.to_string() } else { format!("{} - {}", g.id, g.name) };
                                    ui.selectable_value(&mut selected, Some(g.id), label);
                                }
                                for gid in &self.gscript_group_ids_db {
                                    if self.groups.iter().any(|g| g.id == *gid) { continue; }
                                    ui.selectable_value(&mut selected, Some(*gid), format!("{} [g_script]", gid));
                                }
                            });
                        if selected != self.gscript_group_id {
                            self.gscript_group_id = selected;
                            if let Some(g) = selected { self.load_gscript(g); }
                        }
                        if ui.button("Загрузить").clicked() { if let Some(g) = self.gscript_group_id { self.load_gscript(g); } }
                        if ui.button("Эффективный").clicked() { if let Some(g) = self.gscript_group_id { self.load_effective_gscript(g); } }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Действия:");
                        if ui.button("Проверить").clicked() { self.validate_gscript(); }
                        if ui.button("Сохранить").clicked() { self.save_gscript(); }
                        ui.separator();
                        if ui.button("Разобрать PRE").clicked() { let pre = self.gscript_pre.clone(); self.parse_only(pre.trim(), "PRE"); }
                        if ui.button("Разобрать POST").clicked() { let post = self.gscript_post.clone(); self.parse_only(post.trim(), "POST"); }
                        if ui.button("Запустить PRE").clicked() { let pre = self.gscript_pre.clone(); self.run_gscript(pre.trim(), "PRE"); }
                        if ui.button("Запустить POST").clicked() { let post = self.gscript_post.clone(); self.run_gscript(post.trim(), "POST"); }
                        ui.separator();
                        if ui.button("Показать вывод").clicked() { self.gscript_output_open = true; }
                        if ui.button("Очистить вывод").clicked() { self.clear_gscript_output(); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Шаблон:");
                        let mut selected_tid = self.gscript_template_id;
                        let selected_tname = selected_tid
                            .and_then(|id| self.gscript_templates.iter().find(|t| t.id == id))
                            .map(|t| t.name.clone())
                            .unwrap_or_else(|| "<нет>".to_string());
                        egui::ComboBox::from_id_salt("gscript_template")
                            .selected_text(selected_tname)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut selected_tid, None, "<нет>");
                                for t in &self.gscript_templates {
                                    ui.selectable_value(&mut selected_tid, Some(t.id), format!("{} ({})", t.name, t.id));
                                }
                            });
                        if selected_tid != self.gscript_template_id {
                            self.gscript_template_id = selected_tid;
                            if let Some(id) = selected_tid {
                                if let Some(t) = self.gscript_templates.iter().find(|t| t.id == id) {
                                    self.gscript_template_name = t.name.clone();
                                }
                            }
                        }
                        ui.label("Имя:");
                        ui.add(egui::TextEdit::singleline(&mut self.gscript_template_name).desired_width(180.0));
                        if ui.button("Загрузить шаблон").clicked() { self.load_gscript_template_into_editor(); }
                        if ui.button("Сохранить шаблон").clicked() { self.save_gscript_template(); }
                        if ui.button("Удалить шаблон").clicked() { self.delete_selected_gscript_template(); }
                        if ui.button("Привязать к группе").clicked() { self.bind_template_to_group(); }
                    });

                    if let Some(err) = &self.gscript_err {
                        ui.colored_label(egui::Color32::RED, err);
                    } else if let Some(msg) = &self.gscript_status {
                        ui.colored_label(egui::Color32::GREEN, msg);
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("words (JSON-массив):");
                        ui.add(egui::TextEdit::singleline(&mut self.gscript_words_json).desired_width(320.0));
                        let regs_label = ui.label("Тестовые регистры (JSON):");
                        regs_label.on_hover_text(
                            "Формат: {\"reg_id\": число, ...}\n\n\
                             Эти значения используются при запуске PRE/POST,\n\
                             когда скрипт читает регистры.\n\n\
                             Пример:\n\
                             {\"101\": 12.5, \"205\": 1}\n\n\
                             Расшифровка:\n\
                             - 101: reg_id регистра\n\
                             - 12.5: тестовое значение этого регистра\n\
                             - 205: reg_id регистра\n\
                             - 1: тестовое значение этого регистра\n\n\
                             Еще пример:\n\
                             {\"70010\":1000800000}\n\n\
                             Расшифровка:\n\
                             - 70010: это reg_id регистра\n\
                             - 1000800000: тестовое значение, которое скрипт увидит у этого регистра\n\n\
                             Если такой reg_id есть в JSON, берется именно это значение.\n\
                             Подстановка из arx_val используется только для регистров, которых нет в JSON.",
                        );
                        ui.add(egui::TextEdit::singleline(&mut self.gscript_regs_json).desired_width(360.0));
                        ui.checkbox(&mut self.gscript_hi_lo, "Старшее слово первым");
                        let rv_fallback = ui.checkbox(&mut self.gscript_use_rv_fallback, "Подстановка из arx_val");
                        rv_fallback.on_hover_text(
                            "Если в JSON тестовых регистров указаны не все регистры,\n\
                             недостающие значения будут взяты из таблицы arx_val\n\
                             для выбранного KPZ.\n\n\
                             Это полезно, когда вы хотите подать только часть входов,\n\
                             а остальные оставить из последнего состояния БД.\n\n\
                             Если значение уже есть в JSON тестовых регистров, оно имеет приоритет.",
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.gscript_en, "включен");
                        ui.checkbox(&mut self.gscript_elam, "elam");
                        ui.add(egui::DragValue::new(&mut self.gscript_max_words).speed(1).range(1..=2500));
                        ui.label("max_words");
                        ui.add(egui::DragValue::new(&mut self.gscript_max_k).speed(1).range(1..=16));
                        ui.label("max_k");
                        ui.add(egui::DragValue::new(&mut self.gscript_ver).speed(1));
                        ui.label("версия");
                        if self.gscript_dirty { ui.label("Есть несохраненные изменения"); }
                    });

                    ui.separator();
                    ui.heading("Скрипт");
                    ui.add_space(6.0);
                    {
                        let s = ui.style_mut();
                        s.spacing.scroll = egui::style::ScrollStyle { floating: false, bar_width: 4.0, bar_inner_margin: 2.0, bar_outer_margin: 0.0, ..s.spacing.scroll };
                    }
                    ui.horizontal(|ui| {
                        ui.label("Режим:");
                        let pre = ui.add_sized([64.0, 22.0], egui::SelectableLabel::new(self.gscript_tab == GScriptTab::Pre, "PRE"));
                        if pre.clicked() { self.gscript_tab = GScriptTab::Pre; }
                        let post = ui.add_sized([64.0, 22.0], egui::SelectableLabel::new(self.gscript_tab == GScriptTab::Post, "POST"));
                        if post.clicked() { self.gscript_tab = GScriptTab::Post; }
                    });
                    ui.add_space(6.0);
                    match self.gscript_tab {
                        GScriptTab::Pre => {
                            ui.label("PRE:");
                            egui::ScrollArea::vertical()
                                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                                .drag_to_scroll(false)
                                .max_height(ui.available_height().clamp(260.0, 620.0))
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                                        let job = gscript_layout_job(ui, text, wrap_width);
                                        ui.fonts(|f| f.layout_job(job))
                                    };
                                    let pre = egui::TextEdit::multiline(&mut self.gscript_pre)
                                        .font(egui::TextStyle::Monospace)
                                        .layouter(&mut layouter)
                                        .desired_rows(34)
                                        .desired_width(f32::INFINITY);
                                    if ui.add(pre).changed() { self.gscript_dirty = true; }
                                });
                        }
                        GScriptTab::Post => {
                            ui.label("POST:");
                            egui::ScrollArea::vertical()
                                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                                .drag_to_scroll(false)
                                .max_height(ui.available_height().clamp(260.0, 620.0))
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                                        let job = gscript_layout_job(ui, text, wrap_width);
                                        ui.fonts(|f| f.layout_job(job))
                                    };
                                    let post = egui::TextEdit::multiline(&mut self.gscript_post)
                                        .font(egui::TextStyle::Monospace)
                                        .layouter(&mut layouter)
                                        .desired_rows(34)
                                        .desired_width(f32::INFINITY);
                                    if ui.add(post).changed() { self.gscript_dirty = true; }
                                });
                        }
                    }
                });
            self.gscript_open = open;
        }

        if self.gscript_open && self.gscript_output_open {
            let mut open_out = self.gscript_output_open;
            egui::Window::new("Вывод GScript")
                .open(&mut open_out)
                .resizable(true)
                .default_size([780.0, 520.0])
                .show(ctx, |ui| {
                    ui.heading("Результат");
                    ui.add_space(6.0);
                    {
                        let s = ui.style_mut();
                        s.spacing.scroll = egui::style::ScrollStyle { floating: false, bar_width: 4.0, bar_inner_margin: 2.0, bar_outer_margin: 0.0, ..s.spacing.scroll };
                    }
                    let mut regs_text = if self.gscript_regs_out.is_empty() { "(пусто)".to_string() } else { self.gscript_regs_out.iter().map(|(k, v)| format!("rid={}\t{:.6}", k, v)).collect::<Vec<_>>().join("\n") };
                    let mut emits_text = if self.gscript_emits_out.is_empty() { "(пусто)".to_string() } else { self.gscript_emits_out.iter().map(|(ts, rid, v)| format!("ts={}\trid={}\t{:.6}", ts, rid, v)).collect::<Vec<_>>().join("\n") };
                    ui.horizontal(|ui| {
                        let print = format!("print ({})", self.gscript_print_log.lines().count());
                        if ui.selectable_label(self.gscript_output_tab == GScriptOutputTab::Print, print).clicked() {
                            self.gscript_output_tab = GScriptOutputTab::Print;
                        }
                        let regs = format!("regs ({})", self.gscript_regs_out.len());
                        if ui.selectable_label(self.gscript_output_tab == GScriptOutputTab::Regs, regs).clicked() {
                            self.gscript_output_tab = GScriptOutputTab::Regs;
                        }
                        let emits = format!("emits ({})", self.gscript_emits_out.len());
                        if ui.selectable_label(self.gscript_output_tab == GScriptOutputTab::Emits, emits).clicked() {
                            self.gscript_output_tab = GScriptOutputTab::Emits;
                        }
                    });
                    ui.separator();
                    let (label, text): (&str, &mut String) = match self.gscript_output_tab {
                        GScriptOutputTab::Print => ("Лог print():", &mut self.gscript_print_log),
                        GScriptOutputTab::Regs => ("regs (снимок):", &mut regs_text),
                        GScriptOutputTab::Emits => ("emits:", &mut emits_text),
                    };
                    ui.label(label);
                    egui::ScrollArea::vertical()
                        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                        .drag_to_scroll(false)
                        .max_height(ui.available_height().clamp(260.0, 520.0))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add(egui::TextEdit::multiline(text).font(egui::TextStyle::Monospace).desired_rows(18).interactive(false).desired_width(f32::INFINITY));
                        });
                });
            self.gscript_output_open = open_out;
        }
    }

    pub(crate) fn open_gscript_editor(&mut self) {
        self.reload_gscript_group_ids();
        self.reload_gscript_templates();
        let current_ok = self.gscript_group_id.map(|id| self.groups.iter().any(|g| g.id == id) || self.gscript_group_ids_db.contains(&id)).unwrap_or(false);
        if !current_ok {
            let enabled = self.selected_enabled_groups();
            self.gscript_group_id = enabled.iter().copied().find(|id| self.gscript_group_ids_db.contains(id)).or_else(|| self.gscript_group_ids_db.first().copied()).or_else(|| enabled.first().copied()).or_else(|| self.groups.first().map(|g| g.id));
        }
        self.gscript_open = true;
        self.gscript_output_open = true;
        self.gscript_status = None;
        self.gscript_err = None;
        if let Some(g) = self.gscript_group_id { self.load_gscript(g); }
    }

    pub(crate) fn load_gscript(&mut self, grup: i32) {
        self.clear_gscript_output();
        self.reload_gscript_group_ids();
        self.reload_gscript_templates();
        self.load_group_template_binding(grup);
        match self.db.get_g_script(grup) {
            Ok(Some(row)) => {
                self.gscript_group_id = Some(row.grup);
                self.gscript_pre = row.pre_src;
                self.gscript_post = row.post_src;
                self.gscript_elam = row.elam != 0;
                self.gscript_max_words = row.max_words;
                self.gscript_max_k = row.max_k;
                self.gscript_en = row.en;
                self.gscript_ver = row.ver;
                self.gscript_status = Some("Загружено".to_string());
                self.gscript_err = None;
                self.gscript_dirty = false;
            }
            Ok(None) => {
                self.gscript_pre.clear();
                self.gscript_post.clear();
                self.gscript_elam = false;
                self.gscript_max_words = 800;
                self.gscript_max_k = 2;
                self.gscript_en = true;
                self.gscript_ver = 1;
                self.gscript_status = Some("Новый скрипт (пустой)".to_string());
                self.gscript_err = None;
                self.gscript_dirty = false;
            }
            Err(e) => self.gscript_err = Some(format!("get_g_script failed: {e}")),
        }
    }
    pub(crate) fn reload_gscript_group_ids(&mut self) {
        match self.db.list_g_script_groups() {
            Ok(v) => self.gscript_group_ids_db = v,
            Err(e) => self.gscript_err = Some(format!("list_g_script_groups failed: {e}")),
        }
    }

    pub(crate) fn reload_gscript_templates(&mut self) {
        match self.db.list_g_script_templates() {
            Ok(v) => {
                self.gscript_templates = v;
                let still_exists = self.gscript_template_id.map(|id| self.gscript_templates.iter().any(|t| t.id == id)).unwrap_or(false);
                if !still_exists { self.gscript_template_id = None; }
                if let Some(id) = self.gscript_template_id {
                    if let Some(t) = self.gscript_templates.iter().find(|t| t.id == id) {
                        self.gscript_template_name = t.name.clone();
                    }
                }
            }
            Err(e) => self.gscript_err = Some(format!("list_g_script_templates failed: {e}")),
        }
    }

    pub(crate) fn load_group_template_binding(&mut self, group_id: i32) {
        match self.db.get_group_template_id(group_id) {
            Ok(tid) => {
                self.gscript_template_id = tid;
                if let Some(id) = tid {
                    if let Some(t) = self.gscript_templates.iter().find(|t| t.id == id) {
                        self.gscript_template_name = t.name.clone();
                    }
                } else {
                    self.gscript_template_name.clear();
                }
            }
            Err(e) => self.gscript_err = Some(format!("get_group_template_id failed: {e}")),
        }
    }

    pub(crate) fn load_gscript_template_into_editor(&mut self) {
        let Some(id) = self.gscript_template_id else { self.gscript_err = Some("Шаблон не выбран".to_string()); return; };
        let Some(t) = self.gscript_templates.iter().find(|t| t.id == id).cloned() else { self.gscript_err = Some("Шаблон не найден в кэше".to_string()); return; };
        self.gscript_pre = t.pre_src;
        self.gscript_post = t.post_src;
        self.gscript_elam = t.elam != 0;
        self.gscript_max_words = t.max_words;
        self.gscript_max_k = t.max_k;
        self.gscript_en = t.en;
        self.gscript_ver = t.ver;
        self.gscript_template_name = t.name;
        self.gscript_status = Some("Шаблон загружен в редактор".to_string());
        self.gscript_err = None;
        self.gscript_dirty = true;
    }

    pub(crate) fn save_gscript_template(&mut self) {
        if !self.validate_gscript() { return; }
        let name = self.gscript_template_name.trim().to_string();
        if name.is_empty() { self.gscript_err = Some("Имя шаблона пустое".to_string()); return; }
        let row = GScriptTemplateRow { id: self.gscript_template_id.unwrap_or(0), name: name.clone(), pre_src: self.gscript_pre.clone(), post_src: self.gscript_post.clone(), elam: if self.gscript_elam { 1 } else { 0 }, max_words: self.gscript_max_words, max_k: self.gscript_max_k, en: self.gscript_en, ver: self.gscript_ver };
        match self.db.upsert_g_script_template(&row) {
            Ok(id) => {
                self.gscript_template_id = Some(id);
                self.gscript_template_name = name;
                self.reload_gscript_templates();
                self.gscript_status = Some("Шаблон сохранен".to_string());
                self.gscript_err = None;
            }
            Err(e) => self.gscript_err = Some(format!("upsert_g_script_template failed: {e}")),
        }
    }

    pub(crate) fn delete_selected_gscript_template(&mut self) {
        let Some(id) = self.gscript_template_id else { self.gscript_err = Some("Шаблон не выбран".to_string()); return; };
        if let Err(e) = self.db.delete_g_script_template(id) { self.gscript_err = Some(format!("delete_g_script_template failed: {e}")); return; }
        self.gscript_template_id = None;
        self.gscript_template_name.clear();
        self.reload_gscript_templates();
        self.gscript_status = Some("Шаблон удален".to_string());
        self.gscript_err = None;
    }

    pub(crate) fn bind_template_to_group(&mut self) {
        let Some(group_id) = self.gscript_group_id else { self.gscript_err = Some("Группа не выбрана".to_string()); return; };
        if let Err(e) = self.db.set_group_template(group_id, self.gscript_template_id) { self.gscript_err = Some(format!("set_group_template failed: {e}")); return; }
        self.gscript_status = Some(match self.gscript_template_id { Some(id) => format!("Шаблон {} привязан к группе {}", id, group_id), None => format!("Привязка шаблона очищена для группы {}", group_id) });
        self.gscript_err = None;
    }

    pub(crate) fn load_effective_gscript(&mut self, grup: i32) {
        self.clear_gscript_output();
        match self.db.get_effective_g_script(grup) {
            Ok(Some(row)) => {
                self.gscript_group_id = Some(row.grup);
                self.gscript_pre = row.pre_src;
                self.gscript_post = row.post_src;
                self.gscript_elam = row.elam != 0;
                self.gscript_max_words = row.max_words;
                self.gscript_max_k = row.max_k;
                self.gscript_en = row.en;
                self.gscript_ver = row.ver;
                self.gscript_status = Some("Загружен эффективный скрипт".to_string());
                self.gscript_err = None;
                self.gscript_dirty = false;
            }
            Ok(None) => self.gscript_err = Some("Нет прямого скрипта и нет привязки шаблона".to_string()),
            Err(e) => self.gscript_err = Some(format!("get_effective_g_script failed: {e}")),
        }
    }
    pub(crate) fn parse_words_json(&self) -> Result<Vec<u16>, String> {
        let s = self.gscript_words_json.trim();
        if s.is_empty() { return Ok(Vec::new()); }
        let v: Value = serde_json::from_str(s).map_err(|e| format!("Ошибка разбора words JSON: {e}"))?;
        let arr = v.as_array().ok_or_else(|| "words должен быть JSON-массивом".to_string())?;
        let mut out = Vec::with_capacity(arr.len());
        for (i, it) in arr.iter().enumerate() {
            let n = it.as_i64().or_else(|| it.as_f64().map(|f| f as i64)).ok_or_else(|| format!("words[{i}] должен быть числом"))?;
            if n < 0 || n > u16::MAX as i64 { return Err(format!("words[{i}] вне диапазона 0..65535")); }
            out.push(n as u16);
        }
        Ok(out)
    }

    pub(crate) fn parse_regs_json(&self) -> Result<HashMap<i32, f64>, String> {
        let s = self.gscript_regs_json.trim();
        if s.is_empty() { return Ok(HashMap::new()); }
        let v: Value = serde_json::from_str(s).map_err(|e| format!("Ошибка разбора regs JSON: {e}"))?;
        let obj = v.as_object().ok_or_else(|| "regs должен быть JSON-объектом".to_string())?;
        let mut out = HashMap::new();
        for (k, v) in obj {
            let key = k.parse::<i32>().map_err(|_| format!("reg id не целое число: {k}"))?;
            let val = v.as_f64().or_else(|| v.as_i64().map(|n| n as f64)).ok_or_else(|| format!("значение регистра должно быть числом: {k}"))?;
            out.insert(key, val);
        }
        Ok(out)
    }

    pub(crate) fn clear_gscript_output(&mut self) {
        self.gscript_print_log.clear();
        self.gscript_regs_out.clear();
        self.gscript_emits_out.clear();
    }

    pub(crate) fn run_gscript(&mut self, src: &str, label: &str) {
        self.gscript_err = None;
        self.gscript_status = None;
        self.gscript_output_open = true;
        self.clear_gscript_output();
        let mut rv_map: HashMap<i32, f64> = HashMap::new();
        if self.gscript_use_rv_fallback {
            if let Some(kpz_id) = self.selected_kpz {
                match self.db.get_last_arx_vals(kpz_id) {
                    Ok(v) => rv_map = v,
                    Err(e) => { self.gscript_err = Some(format!("Не удалось загрузить подстановку из arx_val: {e}")); return; }
                }
            }
        }
        let words = match self.parse_words_json() { Ok(v) => v, Err(e) => { self.gscript_err = Some(e); return; } };
        let regs_in = match self.parse_regs_json() { Ok(v) => v, Err(e) => { self.gscript_err = Some(e); return; } };
        let script = match Script::parse(src) { Ok(s) => s, Err(e) => { self.gscript_err = Some(format!("{label} parse: {e}")); return; } };
        let print_buf = RefCell::new(String::new());
        let emits: RefCell<Vec<(f64, i32, f64)>> = RefCell::new(Vec::new());
        let res = script.eval_result(
            &words,
            self.gscript_hi_lo,
            &|rid| *regs_in.get(&rid).or_else(|| rv_map.get(&rid)).unwrap_or(&0.0),
            &|_, _| 0.0,
            Some(&|m| { let mut b = print_buf.borrow_mut(); b.push_str(m); b.push('\n'); }),
            Some(&|ts, reg_id, value| { emits.borrow_mut().push((ts, reg_id, value)); }),
            100_000,
        );
        match res {
            Ok(r) => {
                let mut regs: Vec<(i32, f64)> = r.regs.into_iter().collect();
                regs.sort_by_key(|(k, _)| *k);
                self.gscript_regs_out = regs;
                self.gscript_emits_out = emits.borrow().clone();
                self.gscript_print_log = print_buf.borrow().clone();
                self.gscript_output_tab = if !self.gscript_regs_out.is_empty() {
                    GScriptOutputTab::Regs
                } else if !self.gscript_emits_out.is_empty() {
                    GScriptOutputTab::Emits
                } else {
                    GScriptOutputTab::Print
                };
                self.gscript_status = Some(format!("OK: выполнен {label}. regs={}, emits={}", self.gscript_regs_out.len(), self.gscript_emits_out.len()));
            }
            Err(e) => self.gscript_err = Some(format!("{label} eval: {e}")),
        }
    }

    pub(crate) fn parse_only(&mut self, src: &str, label: &str) {
        self.gscript_err = None;
        self.gscript_status = None;
        match Script::parse(src) {
            Ok(_) => self.gscript_status = Some(format!("OK: {label} разобран")),
            Err(e) => self.gscript_err = Some(format!("{label} parse: {e}")),
        }
    }

    pub(crate) fn validate_gscript(&mut self) -> bool {
        self.gscript_err = None;
        let mut errs = Vec::new();
        if self.gscript_max_words < 1 || self.gscript_max_words > 2500 { errs.push("max_words вне диапазона (1..2500)".to_string()); }
        if self.gscript_max_k < 1 || self.gscript_max_k > 16 { errs.push("max_k вне диапазона (1..16)".to_string()); }
        if self.gscript_pre.trim().is_empty() && self.gscript_post.trim().is_empty() { errs.push("оба скрипта PRE и POST пустые".to_string()); }
        if !self.gscript_pre.trim().is_empty() { if let Err(e) = Script::parse(self.gscript_pre.trim()) { errs.push(format!("PRE: {e}")); } }
        if !self.gscript_post.trim().is_empty() { if let Err(e) = Script::parse(self.gscript_post.trim()) { errs.push(format!("POST: {e}")); } }
        if errs.is_empty() { self.gscript_status = Some("Проверка пройдена".to_string()); true } else { self.gscript_err = Some(errs.join(" | ")); false }
    }

    pub(crate) fn save_gscript(&mut self) {
        let Some(grup) = self.gscript_group_id else { self.gscript_err = Some("Группа не выбрана".to_string()); return; };
        if !self.validate_gscript() { return; }
        let row = GScriptRow { grup, elam: if self.gscript_elam { 1 } else { 0 }, max_words: self.gscript_max_words, max_k: self.gscript_max_k, pre_src: self.gscript_pre.clone(), post_src: self.gscript_post.clone(), en: self.gscript_en, ver: self.gscript_ver };
        if let Err(e) = self.db.upsert_g_script(&row) { self.gscript_err = Some(format!("upsert_g_script failed: {e}")); return; }
        self.reload_gscript_group_ids();
        self.load_group_template_binding(grup);
        self.gscript_status = Some("Сохранено".to_string());
        self.gscript_dirty = false;
    }
}

