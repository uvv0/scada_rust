use eframe::egui;

use crate::app::Ss7App;

pub fn show_script_editor_window(app: &mut Ss7App, ctx: &egui::Context) {
    if !app.script_editor_open {
        return;
    }

    let mut open = app.script_editor_open;
    egui::Window::new("Редактор GScript")
        .open(&mut open)
        .resizable(true)
        .default_size([1100.0, 720.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Обновить").clicked() {
                    app.reload_g_scripts();
                }
                if ui.button("Новый").clicked() {
                    app.new_g_script_form();
                }
                if ui.button("Разобрать PRE").clicked() {
                    app.parse_g_script_pre();
                }
                if ui.button("Разобрать POST").clicked() {
                    app.parse_g_script_post();
                }
                if ui.button("Запустить PRE").clicked() {
                    app.dry_run_g_script_pre();
                }
                if ui.button("Запустить POST").clicked() {
                    app.dry_run_g_script_post();
                }
                if ui.button("Показать вывод").clicked() {
                    app.script_output_open = true;
                }
                if ui.button("Очистить вывод").clicked() {
                    app.clear_script_run_output();
                }
                if ui.button("Сохранить").clicked() {
                    app.save_g_script();
                }
                if ui.small_button("?").clicked() {
                    app.script_help_open = true;
                }
            });

            if let Some(err) = &app.script_err {
                ui.colored_label(egui::Color32::RED, err);
            } else if let Some(status) = &app.script_status {
                ui.colored_label(egui::Color32::GREEN, status);
            }
            ui.separator();

            ui.columns(2, |cols| {
                cols[0].heading("Строки g_script");
                egui::ScrollArea::vertical()
                    .max_height(620.0)
                    .show(&mut cols[0], |ui| {
                        let mut clicked = None;
                        for row in &app.script_rows {
                            let label = format!(
                                "группа={} {} max_words={} max_k={} PRE={} POST={}",
                                row.grup,
                                if row.en { "вкл" } else { "выкл" },
                                row.max_words,
                                row.max_k,
                                row.pre_src.trim().len(),
                                row.post_src.trim().len()
                            );
                            if ui
                                .selectable_label(app.script_selected_group == Some(row.grup), label)
                                .clicked()
                            {
                                clicked = Some(row.grup);
                            }
                        }
                        if let Some(group) = clicked {
                            app.script_selected_group = Some(group);
                            app.sync_script_form_from_selected();
                        }
                    });

                cols[1].heading("Редактор");
                cols[1].horizontal(|ui| {
                    ui.label("grup:");
                    if ui.add(egui::TextEdit::singleline(&mut app.script_grup_input).desired_width(70.0)).changed() {
                        app.script_dirty = true;
                    }
                    let mut elam = app.script_elam_input.trim().parse::<i32>().unwrap_or(0) != 0;
                    if ui.checkbox(&mut elam, "elam").changed() {
                        app.script_elam_input = if elam { "1" } else { "0" }.to_string();
                        app.script_dirty = true;
                    }
                    ui.label("max_words:");
                    let mut max_words = app.script_max_words_input.trim().parse::<i32>().unwrap_or(800).clamp(1, 2500);
                    if ui.add(egui::DragValue::new(&mut max_words).speed(1).range(1..=2500)).changed() {
                        app.script_max_words_input = max_words.to_string();
                        app.script_dirty = true;
                    }
                    ui.label("max_k:");
                    let mut max_k = app.script_max_k_input.trim().parse::<i32>().unwrap_or(2).clamp(1, 16);
                    if ui.add(egui::DragValue::new(&mut max_k).speed(1).range(1..=16)).changed() {
                        app.script_max_k_input = max_k.to_string();
                        app.script_dirty = true;
                    }
                    ui.label("версия:");
                    let mut ver = app.script_ver_input.trim().parse::<i32>().unwrap_or(1);
                    if ui.add(egui::DragValue::new(&mut ver).speed(1)).changed() {
                        app.script_ver_input = ver.to_string();
                        app.script_dirty = true;
                    }
                    if ui.checkbox(&mut app.script_enabled, "включен").changed() {
                        app.script_dirty = true;
                    }
                    if app.script_dirty {
                        ui.colored_label(egui::Color32::YELLOW, "Есть несохраненные изменения");
                    }
                });
                cols[1].separator();
                cols[1].horizontal(|ui| {
                    ui.selectable_value(&mut app.script_editor_tab, 0, "PRE");
                    ui.selectable_value(&mut app.script_editor_tab, 1, "POST");
                });
                cols[1].separator();
                if app.script_editor_tab > 1 {
                    app.script_editor_tab = 0;
                }
                if app.script_editor_tab == 0 {
                    cols[1].label("PRE:");
                    if cols[1].add(
                        egui::TextEdit::multiline(&mut app.script_pre_src)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(34),
                    ).changed() {
                        app.script_dirty = true;
                    }
                } else {
                    cols[1].label("POST:");
                    if cols[1].add(
                        egui::TextEdit::multiline(&mut app.script_post_src)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(34),
                    ).changed() {
                        app.script_dirty = true;
                    }
                }
                cols[1].separator();
                cols[1].label("DSL: let/if/else/while/for, reg(id)=v, emit(ts,reg,v), rv/u16/i16/u32/i32/f32/bit/av/print/print2/dt2unix/min/max/clamp/floor.");
            });
        });
    app.script_editor_open = open;

    if app.script_editor_open && app.script_output_open {
        let mut output_open = app.script_output_open;
        egui::Window::new("Вывод GScript")
            .open(&mut output_open)
            .resizable(true)
            .default_size([780.0, 520.0])
            .show(ctx, |ui| {
                ui.heading("Результат");
                ui.add_space(6.0);
                let mut regs_text = if app.script_regs_out.is_empty() {
                    "(пусто)".to_string()
                } else {
                    app.script_regs_out
                        .iter()
                        .map(|(reg_id, value)| format!("rid={reg_id}\t{value:.6}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                let mut emits_text = if app.script_emits_out.is_empty() {
                    "(пусто)".to_string()
                } else {
                    app.script_emits_out
                        .iter()
                        .map(|(ts, reg_id, value)| format!("ts={ts:.0}\trid={reg_id}\t{value:.6}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                ui.horizontal(|ui| {
                    let print = format!("print ({})", app.script_print_log.lines().count());
                    if ui.selectable_label(app.script_output_tab == 0, print).clicked() {
                        app.script_output_tab = 0;
                    }
                    let regs = format!("regs ({})", app.script_regs_out.len());
                    if ui.selectable_label(app.script_output_tab == 1, regs).clicked() {
                        app.script_output_tab = 1;
                    }
                    let emits = format!("emits ({})", app.script_emits_out.len());
                    if ui.selectable_label(app.script_output_tab == 2, emits).clicked() {
                        app.script_output_tab = 2;
                    }
                });
                ui.separator();
                let (label, text): (&str, &mut String) = match app.script_output_tab {
                    1 => ("regs (снимок):", &mut regs_text),
                    2 => ("emits:", &mut emits_text),
                    _ => ("Лог print():", &mut app.script_print_log),
                };
                ui.label(label);
                egui::ScrollArea::vertical()
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .drag_to_scroll(false)
                    .max_height(ui.available_height().clamp(260.0, 520.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(text)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(18)
                                .interactive(false),
                        );
                    });
            });
        app.script_output_open = output_open;
    }

    if app.script_help_open {
        let mut help_open = app.script_help_open;
        egui::Window::new("Редактор GScript - справка")
            .open(&mut help_open)
            .resizable(true)
            .default_size([760.0, 620.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Рабочий порядок");
                    ui.label("Обновить: перечитать строки g_script из БД.");
                    ui.label("Новый: очистить форму для нового скрипта группы.");
                    ui.label("Разобрать PRE/POST: проверить синтаксис выбранной части тем же parser-ом, который использует scheduler.");
                    ui.label("Запустить PRE/POST: безопасно выполнить выбранную часть с нулевыми rv()/av() и тестовым буфером words. Не пишет в БД и не отправляет Modbus.");
                    ui.label("Сохранить: сохранить текущую форму в g_script через upsert.");
                    ui.separator();
                    ui.heading("PRE/POST");
                    ui.label("PRE рассчитывает команды чтения через запись reg(1000..). Scheduler разбирает эти command-регистры и выполняет Modbus-чтение.");
                    ui.label("POST получает words из ответа и может писать рассчитанные значения через reg(id)=value, а архивные точки через emit(ts, reg_id, value).");
                    ui.separator();
                    ui.heading("Операторы");
                    ui.label("let name = expr; объявляет или присваивает переменную. В существующих скриптах let часто повторно используется для присваивания.");
                    ui.label("name = expr; присваивает значение переменной.");
                    ui.label("if (cond) stmt else stmt, while (cond) stmt, for (init; cond; step) stmt.");
                    ui.label("reg(expr) = expr; записывает выходное значение регистра.");
                    ui.label("emit(ts, reg_id, value); формирует архивное/event-значение из POST.");
                    ui.separator();
                    ui.heading("Функции");
                    ui.label("rv(key): runtime value / контекстный регистр.");
                    ui.label("u16(i), i16(i): прочитать одно word-значение ответа по смещению i.");
                    ui.label("u32(i), i32(i), f32(i): прочитать 32-битное значение из words ответа.");
                    ui.label("bit(i, b): прочитать бит b из word i.");
                    ui.label("av(kpz, reg): последнее архивное значение, если runtime передал callback.");
                    ui.label("dt2unix(raw): преобразовать дату/время устройства в unix timestamp.");
                    ui.label("print(x), print2(key, value): отладочный trace.");
                    ui.label("abs/sqrt/floor/ceil/round/min/max/pow/clamp: математические функции.");
                    ui.separator();
                    ui.heading("Ограничения");
                    ui.label("Тестовый запуск и scheduler используют лимит VM-шагов, чтобы останавливать случайные бесконечные циклы.");
                    ui.label("max ограничивает количество Modbus words; max_k ограничивает количество команд скрипта.");
                });
            });
        app.script_help_open = help_open;
    }
}
