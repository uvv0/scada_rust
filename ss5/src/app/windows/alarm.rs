use crate::app::Ss5App;
use eframe::egui;

impl Ss5App {
    fn alarm_cmp_label(cmp: &str) -> &'static str {
        match cmp {
            "lt" => "lt - меньше",
            "le" => "le - меньше или равно",
            "gt" => "gt - больше",
            "ge" => "ge - больше или равно",
            "lt_1" => "lt_1 - меньше lvl1",
            "le_1" => "le_1 - меньше или равно lvl1",
            "gt_1" => "gt_1 - больше lvl1",
            "ge_1" => "ge_1 - больше или равно lvl1",
            "between" => "between - внутри диапазона",
            "outside" => "outside - вне диапазона",
            _ => "cmp",
        }
    }

    fn alarm_severity_label(severity: &str) -> &'static str {
        match severity.trim() {
            "1" => "1 - предупреждение",
            "2" => "2 - авария",
            "3" => "3 - критично",
            _ => "severity",
        }
    }

    pub(crate) fn open_alarm_window(&mut self) {
        self.reload_alarm_data();
        self.alarm_open = true;
    }

    fn reload_alarm_data(&mut self) {
        let filter = self.effective_kpz_filter();
        match self.db.get_alarm_rules(filter) {
            Ok(v) => {
                self.alarm_rules = v;
                if self.alarm_selected_rule_id.is_none() {
                    self.alarm_selected_rule_id = self.alarm_rules.first().map(|r| r.id);
                } else if let Some(id) = self.alarm_selected_rule_id {
                    if !self.alarm_rules.iter().any(|r| r.id == id) {
                        self.alarm_selected_rule_id = self.alarm_rules.first().map(|r| r.id);
                    }
                }
                self.sync_alarm_editor_from_selected();
            }
            Err(e) => self.alarm_err = Some(format!("get_alarm_rules failed: {e}")),
        }
        match self.db.get_alarm_state(filter, self.alarm_state_limit) {
            Ok(v) => self.alarm_state_rows = v,
            Err(e) => self.alarm_err = Some(format!("get_alarm_state failed: {e}")),
        }
        match self.db.get_alarm_events(filter, self.alarm_events_limit) {
            Ok(v) => self.alarm_events = v,
            Err(e) => self.alarm_err = Some(format!("get_alarm_events failed: {e}")),
        }
    }

    fn sync_alarm_editor_from_selected(&mut self) {
        self.alarm_group_id = None;
        self.alarm_group_regs.clear();
        if let Some(row) = self
            .alarm_selected_rule_id
            .and_then(|id| self.alarm_rules.iter().find(|r| r.id == id))
        {
            self.alarm_rule_kpz_id = row.kpz_id.to_string();
            self.alarm_rule_reg_id = row.reg_id.to_string();
            let groups = self.graph_groups_for_kpz(row.kpz_id);
            for g in groups {
                if let Ok(regs) = self.db.get_regs_for_group(g.id) {
                    if regs.iter().any(|r| r.id == row.reg_id) {
                        self.alarm_group_id = Some(g.id);
                        self.alarm_group_regs = regs;
                        break;
                    }
                }
            }
            self.alarm_rule_enabled = row.enabled;
            self.alarm_rule_cmp = row.cmp.clone();
            self.alarm_rule_set_lo = row.set_lo.map(|v| v.to_string()).unwrap_or_default();
            self.alarm_rule_set_hi = row.set_hi.map(|v| v.to_string()).unwrap_or_default();
            self.alarm_rule_set_lo_1 = row.set_lo_1.map(|v| v.to_string()).unwrap_or_default();
            self.alarm_rule_set_hi_1 = row.set_hi_1.map(|v| v.to_string()).unwrap_or_default();
            self.alarm_rule_hysteresis = row.hysteresis.to_string();
            self.alarm_rule_on_delay = row.on_delay_sec.to_string();
            self.alarm_rule_off_delay = row.off_delay_sec.to_string();
            self.alarm_rule_severity = row.severity.to_string();
            self.alarm_rule_code = row.code.clone().unwrap_or_default();
            self.alarm_rule_message = row.message.clone().unwrap_or_default();
            self.alarm_rule_chat_id = row.chat_id.clone().unwrap_or_default();
            self.alarm_tg_on_on = row.tg_on_on;
            self.alarm_tg_on_off = row.tg_on_off;
            self.alarm_tg_thr_main = row.tg_thr_main;
            self.alarm_tg_thr_lvl1 = row.tg_thr_lvl1;
        } else {
            self.new_alarm_rule_form();
        }
        self.alarm_err = None;
        self.alarm_status = None;
    }

    fn new_alarm_rule_form(&mut self) {
        self.alarm_selected_rule_id = None;
        if self.alarm_rule_kpz_id.trim().is_empty() {
            self.alarm_rule_kpz_id = self.selected_kpz.map(|v| v.to_string()).unwrap_or_default();
        }
        if self.alarm_group_id.is_some() && self.alarm_group_regs.is_empty() {
            self.reload_alarm_group_regs();
        }
        self.alarm_err = None;
        self.alarm_status = None;
    }

    fn reload_alarm_group_regs(&mut self) {
        self.alarm_group_regs.clear();
        let Some(group_id) = self.alarm_group_id else {
            return;
        };
        match self.db.get_regs_for_group(group_id) {
            Ok(v) => self.alarm_group_regs = v,
            Err(e) => self.alarm_err = Some(format!("get_regs_for_group failed: {e}")),
        }
    }

    fn parse_opt_f64_field(raw: &str) -> anyhow::Result<Option<f64>> {
        let s = raw.trim();
        if s.is_empty() {
            return Ok(None);
        }
        match s.parse::<f64>() {
            Ok(v) => Ok(Some(v)),
            Err(_) => Err(anyhow::anyhow!("bad number: {}", s)),
        }
    }

    fn save_alarm_rule(&mut self) {
        let kpz_id = match self.alarm_rule_kpz_id.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.alarm_err = Some("kpz_id должен быть целым числом".to_string());
                return;
            }
        };
        let reg_id = match self.alarm_rule_reg_id.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                self.alarm_err = Some("reg_id должен быть целым числом".to_string());
                return;
            }
        };
        let cmp = self.alarm_rule_cmp.trim().to_lowercase();
        if ![
            "lt", "le", "gt", "ge", "lt_1", "le_1", "gt_1", "ge_1", "between", "outside",
        ]
        .contains(&cmp.as_str())
        {
            self.alarm_err =
                Some("cmp должен быть одним из: lt le gt ge lt_1 le_1 gt_1 ge_1 between outside".to_string());
            return;
        }
        let set_lo = match Self::parse_opt_f64_field(&self.alarm_rule_set_lo) {
            Ok(v) => v,
            Err(e) => {
                self.alarm_err = Some(format!("set_lo {e}"));
                return;
            }
        };
        let set_hi = match Self::parse_opt_f64_field(&self.alarm_rule_set_hi) {
            Ok(v) => v,
            Err(e) => {
                self.alarm_err = Some(format!("set_hi {e}"));
                return;
            }
        };
        let set_lo_1 = match Self::parse_opt_f64_field(&self.alarm_rule_set_lo_1) {
            Ok(v) => v,
            Err(e) => {
                self.alarm_err = Some(format!("set_lo_1 {e}"));
                return;
            }
        };
        let set_hi_1 = match Self::parse_opt_f64_field(&self.alarm_rule_set_hi_1) {
            Ok(v) => v,
            Err(e) => {
                self.alarm_err = Some(format!("set_hi_1 {e}"));
                return;
            }
        };
        if let (Some(lo), Some(lo_1)) = (set_lo, set_lo_1) {
            if lo_1 <= lo {
                self.alarm_err = Some("set_lo_1 должен быть > set_lo".to_string());
                return;
            }
        }
        if let (Some(hi), Some(hi_1)) = (set_hi, set_hi_1) {
            if hi_1 >= hi {
                self.alarm_err = Some("set_hi_1 должен быть < set_hi".to_string());
                return;
            }
        }
        let hysteresis = match self.alarm_rule_hysteresis.trim().parse::<f64>() {
            Ok(v) => v.max(0.0),
            Err(_) => {
                self.alarm_err = Some("hysteresis должен быть числом".to_string());
                return;
            }
        };
        let on_delay_sec = match self.alarm_rule_on_delay.trim().parse::<i32>() {
            Ok(v) if v >= 0 => v,
            _ => {
                self.alarm_err = Some("on_delay_sec должен быть целым числом >= 0".to_string());
                return;
            }
        };
        let off_delay_sec = match self.alarm_rule_off_delay.trim().parse::<i32>() {
            Ok(v) if v >= 0 => v,
            _ => {
                self.alarm_err = Some("off_delay_sec должен быть целым числом >= 0".to_string());
                return;
            }
        };
        let severity = match self.alarm_rule_severity.trim().parse::<i16>() {
            Ok(v) => v,
            Err(_) => {
                self.alarm_err = Some("severity должен быть целым числом".to_string());
                return;
            }
        };
        let code = {
            let s = self.alarm_rule_code.trim();
            if s.is_empty() { None } else { Some(s.to_string()) }
        };
        let message = {
            let s = self.alarm_rule_message.trim();
            if s.is_empty() { None } else { Some(s.to_string()) }
        };
        let chat_id = {
            let s = self.alarm_rule_chat_id.trim();
            if s.is_empty() { None } else { Some(s.to_string()) }
        };

        let mut row = crate::models::AlarmRuleRow {
            id: self.alarm_selected_rule_id.unwrap_or(0),
            kpz_id,
            reg_id,
            enabled: self.alarm_rule_enabled,
            cmp,
            set_lo,
            set_hi,
            set_lo_1,
            set_hi_1,
            hysteresis,
            on_delay_sec,
            off_delay_sec,
            severity,
            code,
            message,
            chat_id,
            tg_on_on: self.alarm_tg_on_on,
            tg_on_off: self.alarm_tg_on_off,
            tg_thr_main: self.alarm_tg_thr_main,
            tg_thr_lvl1: self.alarm_tg_thr_lvl1,
        };

        let saved_id = if row.id > 0 {
            match self.db.upsert_alarm_rule(&row) {
                Ok(v) => v,
                Err(e) => {
                    self.alarm_err = Some(format!("upsert_alarm_rule failed: {e}"));
                    return;
                }
            }
        } else {
            match self.db.insert_alarm_rule(&row) {
                Ok(v) => v,
                Err(e) => {
                    self.alarm_err = Some(format!("insert_alarm_rule failed: {e}"));
                    return;
                }
            }
        };

        row.id = saved_id;
        self.alarm_selected_rule_id = Some(saved_id);
        self.reload_alarm_data();
        self.alarm_status = Some("Сохранено".to_string());
        self.alarm_err = None;
        self.push_log(format!("Правило тревоги {} сохранено", saved_id));
    }

    fn delete_alarm_rule(&mut self) {
        let Some(id) = self.alarm_selected_rule_id else {
            self.alarm_err = Some("Правило тревоги не выбрано".to_string());
            return;
        };
        if let Err(e) = self.db.delete_alarm_rule(id) {
            self.alarm_err = Some(format!("delete_alarm_rule failed: {e}"));
            return;
        }
        self.reload_alarm_data();
        self.new_alarm_rule_form();
        self.alarm_status = Some("Удалено".to_string());
        self.alarm_err = None;
        self.push_log(format!("Правило тревоги {} удалено", id));
    }

    pub(crate) fn show_alarm_window(&mut self, ctx: &egui::Context) {
        if !self.alarm_open {
            return;
        }

        let mut open = self.alarm_open;
        egui::Window::new("Тревоги")
            .open(&mut open)
            .resizable(true)
            .default_size([1220.0, 760.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Фильтр KPZ: {}", self.selected_kpz_name()));
                    if ui.button("Обновить").clicked() {
                        self.reload_alarm_data();
                    }
                    if ui.button("Новое").clicked() {
                        self.new_alarm_rule_form();
                    }
                    if ui.button("Сохранить").clicked() {
                        self.save_alarm_rule();
                    }
                    if ui.button("Удалить").clicked() {
                        self.delete_alarm_rule();
                    }
                });

                if let Some(err) = &self.alarm_err {
                    ui.colored_label(egui::Color32::RED, err);
                } else if let Some(msg) = &self.alarm_status {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }
                ui.separator();

                ui.columns(2, |cols| {
                    cols[0].heading("Правила");
                    egui::ScrollArea::vertical().max_height(260.0).show(&mut cols[0], |ui| {
                        let mut clicked_id: Option<i64> = None;
                        for row in &self.alarm_rules {
                            let label = format!("{} | kpz={} reg={} {}", row.id, row.kpz_id, row.reg_id, row.cmp);
                            if ui
                                .selectable_label(self.alarm_selected_rule_id == Some(row.id), label)
                                .clicked()
                            {
                                clicked_id = Some(row.id);
                            }
                        }
                        if let Some(id) = clicked_id {
                            self.alarm_selected_rule_id = Some(id);
                            self.sync_alarm_editor_from_selected();
                        }
                    });

                    cols[1].heading("Редактор");
                    cols[1].horizontal(|ui| {
                        ui.label("kpz_id:");
                        ui.add(egui::TextEdit::singleline(&mut self.alarm_rule_kpz_id).desired_width(80.0));
                        ui.label("group:");
                        let mut gid = self.alarm_group_id;
                        let groups = self.graph_groups_for_selected_kpz();
                        egui::ComboBox::from_id_salt("alarm_group_id")
                            .selected_text(gid.map(|v| v.to_string()).unwrap_or_else(|| "<нет>".to_string()))
                            .show_ui(ui, |ui| {
                                for g in &groups {
                                    let label = if g.name.is_empty() { g.id.to_string() } else { format!("{} - {}", g.id, g.name) };
                                    ui.selectable_value(&mut gid, Some(g.id), label);
                                }
                            });
                        if gid != self.alarm_group_id {
                            self.alarm_group_id = gid;
                            self.reload_alarm_group_regs();
                        }
                    });
                    cols[1].horizontal(|ui| {
                        ui.label("reg_id:");
                        let mut reg_id = self.alarm_rule_reg_id.clone();
                        egui::ComboBox::from_id_salt("alarm_reg_id")
                            .selected_text(if reg_id.is_empty() { "<нет>".to_string() } else { reg_id.clone() })
                            .show_ui(ui, |ui| {
                                for r in &self.alarm_group_regs {
                                    ui.selectable_value(&mut reg_id, r.id.to_string(), format!("{} - {}", r.id, r.name));
                                }
                            });
                        self.alarm_rule_reg_id = reg_id;
                        ui.checkbox(&mut self.alarm_rule_enabled, "Включено");
                    });
                    cols[1].horizontal(|ui| {
                        ui.label("cmp:");
                        egui::ComboBox::from_id_salt("alarm_cmp")
                            .selected_text(Self::alarm_cmp_label(&self.alarm_rule_cmp))
                            .width(220.0)
                            .show_ui(ui, |ui| {
                                for cmp in [
                                    "lt",
                                    "le",
                                    "gt",
                                    "ge",
                                    "lt_1",
                                    "le_1",
                                    "gt_1",
                                    "ge_1",
                                    "between",
                                    "outside",
                                ] {
                                    ui.selectable_value(
                                        &mut self.alarm_rule_cmp,
                                        cmp.to_string(),
                                        Self::alarm_cmp_label(cmp),
                                    );
                                }
                            });
                        if ui.small_button("?").clicked() {
                            self.alarm_status = Some(match self.alarm_rule_cmp.as_str() {
                                "lt" => "cmp: тревога, если значение меньше set_lo".to_string(),
                                "le" => "cmp: тревога, если значение меньше или равно set_lo".to_string(),
                                "gt" => "cmp: тревога, если значение больше set_hi".to_string(),
                                "ge" => "cmp: тревога, если значение больше или равно set_hi".to_string(),
                                "lt_1" => "cmp: тревога по предупреждению, если значение меньше set_lo_1".to_string(),
                                "le_1" => "cmp: тревога по предупреждению, если значение меньше или равно set_lo_1".to_string(),
                                "gt_1" => "cmp: тревога по предупреждению, если значение больше set_hi_1".to_string(),
                                "ge_1" => "cmp: тревога по предупреждению, если значение больше или равно set_hi_1".to_string(),
                                "between" => "cmp: тревога, если значение внутри диапазона set_lo .. set_hi".to_string(),
                                "outside" => "cmp: тревога, если значение вне диапазона set_lo .. set_hi".to_string(),
                                _ => "cmp: условие срабатывания тревоги".to_string(),
                            });
                            self.alarm_err = None;
                        }
                        ui.label("severity:");
                        egui::ComboBox::from_id_salt("alarm_severity")
                            .selected_text(Self::alarm_severity_label(&self.alarm_rule_severity))
                            .width(170.0)
                            .show_ui(ui, |ui| {
                                for sev in ["1", "2", "3"] {
                                    ui.selectable_value(
                                        &mut self.alarm_rule_severity,
                                        sev.to_string(),
                                        Self::alarm_severity_label(sev),
                                    );
                                }
                            });
                        if ui.small_button("?").clicked() {
                            self.alarm_status = Some(
                                "severity: важность тревоги после срабатывания, обычно 1=предупреждение, 2=авария, 3=критично"
                                    .to_string(),
                            );
                            self.alarm_err = None;
                        }
                    });
                    cols[1].horizontal(|ui| {
                        ui.label("set_lo:");
                        ui.add(egui::TextEdit::singleline(&mut self.alarm_rule_set_lo).desired_width(90.0));
                        ui.label("set_hi:");
                        ui.add(egui::TextEdit::singleline(&mut self.alarm_rule_set_hi).desired_width(90.0));
                        ui.label("set_lo_1:");
                        ui.add(egui::TextEdit::singleline(&mut self.alarm_rule_set_lo_1).desired_width(90.0));
                        ui.label("set_hi_1:");
                        ui.add(egui::TextEdit::singleline(&mut self.alarm_rule_set_hi_1).desired_width(90.0));
                    });
                    cols[1].horizontal(|ui| {
                        ui.label("hyst:");
                        ui.add(egui::TextEdit::singleline(&mut self.alarm_rule_hysteresis).desired_width(80.0));
                        ui.label("on_delay:");
                        ui.add(egui::TextEdit::singleline(&mut self.alarm_rule_on_delay).desired_width(80.0));
                        ui.label("off_delay:");
                        ui.add(egui::TextEdit::singleline(&mut self.alarm_rule_off_delay).desired_width(80.0));
                    });
                    cols[1].horizontal(|ui| {
                        ui.label("code:");
                        ui.add(egui::TextEdit::singleline(&mut self.alarm_rule_code).desired_width(140.0));
                        ui.label("chat_id:");
                        ui.add(egui::TextEdit::singleline(&mut self.alarm_rule_chat_id).desired_width(140.0));
                    });
                    cols[1].label("message:");
                    cols[1].add(egui::TextEdit::multiline(&mut self.alarm_rule_message).desired_rows(2).desired_width(f32::INFINITY));
                });

                ui.separator();
                ui.columns(2, |cols| {
                    cols[0].heading("Состояние");
                    egui::ScrollArea::vertical().max_height(180.0).show(&mut cols[0], |ui| {
                        for s in &self.alarm_state_rows {
                            ui.label(format!(
                                "rule={} kpz={} reg={} active={} since={} value={}",
                                s.rule_id,
                                s.kpz_id,
                                s.reg_id,
                                s.active,
                                s.active_since.clone().unwrap_or_else(|| "-".to_string()),
                                s.last_value.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
                            ));
                        }
                    });
                    cols[1].heading("События");
                    egui::ScrollArea::vertical().max_height(180.0).show(&mut cols[1], |ui| {
                        for e in &self.alarm_events {
                            ui.label(format!(
                                "{} rule={} kpz={} reg={} event={} value={}",
                                e.ts,
                                e.rule_id,
                                e.kpz_id,
                                e.reg_id,
                                e.event,
                                e.value.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
                            ));
                        }
                    });
                });
            });
        self.alarm_open = open;
    }
}

