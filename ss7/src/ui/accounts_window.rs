use eframe::egui;

use crate::app::Ss7App;

pub fn show_accounts_window(app: &mut Ss7App, ctx: &egui::Context) {
    if !app.accounts_window_open {
        return;
    }

    let mut open = app.accounts_window_open;
    egui::Window::new("Учетки Web")
        .open(&mut open)
        .resizable(true)
        .default_size([760.0, 420.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Обновить").clicked() {
                    app.reload_web_accounts();
                }
                if ui.button("Новая").clicked() {
                    app.new_web_account_form();
                }
                if ui.button("Сохранить").clicked() {
                    app.save_web_account();
                }
                if ui.button("Удалить").clicked() {
                    app.delete_web_account();
                }
            });

            if let Some(err) = &app.web_account_err {
                ui.colored_label(egui::Color32::RED, err);
            } else if let Some(msg) = &app.web_account_status {
                ui.colored_label(egui::Color32::GREEN, msg);
            }
            ui.separator();

            ui.columns(2, |cols| {
                cols[0].heading("Список");
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(&mut cols[0], |ui| {
                        let mut clicked_id: Option<i64> = None;
                        for row in &app.web_accounts {
                            let label = format!(
                                "{} | {} | {} | {} | kpz={}..{}",
                                row.id,
                                row.login,
                                row.role,
                                if row.enabled { "on" } else { "off" },
                                row.kpz_from.map(|v| v.to_string()).unwrap_or_else(|| "*".to_string()),
                                row.kpz_to.map(|v| v.to_string()).unwrap_or_else(|| "*".to_string())
                            );
                            if ui
                                .selectable_label(app.web_account_selected_id == Some(row.id), label)
                                .clicked()
                            {
                                clicked_id = Some(row.id);
                            }
                        }
                        if let Some(id) = clicked_id {
                            app.web_account_selected_id = Some(id);
                            app.sync_web_account_from_selected();
                        }
                    });

                cols[1].heading("Редактор");
                cols[1].horizontal(|ui| {
                    ui.label("login:");
                    ui.add(egui::TextEdit::singleline(&mut app.web_account_login).desired_width(220.0));
                });
                cols[1].horizontal(|ui| {
                    ui.label("password:");
                    ui.add(
                        egui::TextEdit::singleline(&mut app.web_account_password)
                            .password(true)
                            .hint_text(if app.web_account_selected_id.is_some() {
                                "leave blank to keep current"
                            } else {
                                "required for new account"
                            })
                            .desired_width(220.0),
                    );
                });
                cols[1].horizontal(|ui| {
                    ui.label("role:");
                    egui::ComboBox::from_id_salt("ss7_web_account_role")
                        .selected_text(app.web_account_role.clone())
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            for role in ["admin", "operator", "viewer"] {
                                ui.selectable_value(
                                    &mut app.web_account_role,
                                    role.to_string(),
                                    role,
                                );
                            }
                        });
                    ui.checkbox(&mut app.web_account_enabled, "enabled");
                });
                cols[1].horizontal(|ui| {
                    ui.label("kpz from:");
                    ui.add(egui::TextEdit::singleline(&mut app.web_account_kpz_from).desired_width(90.0));
                    ui.label("to:");
                    ui.add(egui::TextEdit::singleline(&mut app.web_account_kpz_to).desired_width(90.0));
                });
                cols[1].label("Пустые kpz from/to означают полный доступ ко всем КПЗ. Для существующей записи пустой password значит: не менять пароль.");
            });
        });
    app.accounts_window_open = open;
}
