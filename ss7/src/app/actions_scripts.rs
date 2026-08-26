use std::cell::RefCell;

use crate::app::{script_dsl, Ss7App};
use crate::models::GScriptRow;

impl Ss7App {
    pub(crate) fn open_script_editor(&mut self) {
        self.script_editor_open = true;
        self.script_output_open = true;
        if self.script_rows.is_empty() {
            self.reload_g_scripts();
        }
    }

    pub(crate) fn reload_g_scripts(&mut self) {
        match self.db.list_g_scripts() {
            Ok(rows) => {
                self.script_rows = rows;
                self.script_status = Some(format!("загружено строк g_script: {}", self.script_rows.len()));
                self.script_err = None;
                if let Some(group) = self.script_selected_group {
                    if self.script_rows.iter().any(|r| r.grup == group) {
                        self.sync_script_form_from_selected();
                    } else {
                        self.new_g_script_form();
                    }
                }
            }
            Err(e) => {
                self.script_err = Some(format!("не удалось загрузить g_script: {e}"));
            }
        }
    }

    pub(crate) fn new_g_script_form(&mut self) {
        self.script_selected_group = None;
        self.script_grup_input.clear();
        self.script_elam_input = "0".to_string();
        self.script_max_words_input = "800".to_string();
        self.script_max_k_input = "2".to_string();
        self.script_pre_src.clear();
        self.script_post_src.clear();
        self.script_enabled = true;
        self.script_ver_input = "1".to_string();
        self.clear_script_run_output();
        self.script_status = Some("новый черновик скрипта".to_string());
        self.script_err = None;
        self.script_dirty = false;
    }

    pub(crate) fn sync_script_form_from_selected(&mut self) {
        let Some(group) = self.script_selected_group else {
            return;
        };
        let Some(row) = self.script_rows.iter().find(|r| r.grup == group) else {
            return;
        };
        self.script_grup_input = row.grup.to_string();
        self.script_elam_input = row.elam.to_string();
        self.script_max_words_input = row.max_words.to_string();
        self.script_max_k_input = row.max_k.to_string();
        self.script_pre_src = row.pre_src.clone();
        self.script_post_src = row.post_src.clone();
        self.script_enabled = row.en;
        self.script_ver_input = row.ver.to_string();
        self.clear_script_run_output();
        self.script_status = Some(format!("выбрана группа {}", group));
        self.script_err = None;
        self.script_dirty = false;
    }

    pub(crate) fn save_g_script(&mut self) {
        let parse_i32 = |name: &str, value: &str| -> Result<i32, String> {
            value
                .trim()
                .parse::<i32>()
                .map_err(|e| format!("{name}: bad integer: {e}"))
        };
        let row = match (
            parse_i32("grup", &self.script_grup_input),
            parse_i32("elam", &self.script_elam_input),
            parse_i32("max", &self.script_max_words_input),
            parse_i32("max_k", &self.script_max_k_input),
            parse_i32("ver", &self.script_ver_input),
        ) {
            (Ok(grup), Ok(elam), Ok(max_words), Ok(max_k), Ok(ver)) => GScriptRow {
                grup,
                elam,
                max_words: max_words.clamp(1, 2500),
                max_k: max_k.clamp(1, 16),
                pre_src: self.script_pre_src.clone(),
                post_src: self.script_post_src.clone(),
                en: self.script_enabled,
                ver,
            },
            _ => {
                self.script_err = Some("проверьте числовые поля: grup/elam/max/max_k/ver".to_string());
                return;
            }
        };
        match self.db.upsert_g_script(&row) {
            Ok(()) => {
                self.script_selected_group = Some(row.grup);
                self.script_status = Some(format!("сохранена группа {}", row.grup));
                self.script_err = None;
                self.script_dirty = false;
                self.reload_g_scripts();
            }
            Err(e) => {
                self.script_err = Some(format!("не удалось сохранить g_script: {e}"));
            }
        }
    }

    pub(crate) fn parse_g_script_pre(&mut self) {
        self.parse_g_script_one("PRE", &self.script_pre_src.clone());
    }

    pub(crate) fn parse_g_script_post(&mut self) {
        self.parse_g_script_one("POST", &self.script_post_src.clone());
    }

    fn parse_g_script_one(&mut self, label: &str, src: &str) {
        let trimmed = src.trim();
        if trimmed.is_empty() {
            self.script_status = Some(format!("{label} пустой"));
            self.script_err = None;
            return;
        }
        match script_dsl::Script::parse(trimmed) {
            Ok(script) => {
                self.script_status = Some(format!(
                    "{label} ok; rv keys: {}",
                    format_keys(script.used_rv_keys())
                ));
                self.script_err = None;
            }
            Err(e) => {
                self.script_err = Some(format!("{label}: ошибка разбора: {e}"));
            }
        }
    }

    pub(crate) fn dry_run_g_script_pre(&mut self) {
        let src = self.script_pre_src.clone();
        self.dry_run_g_script_one("PRE", &src, Vec::new());
    }

    pub(crate) fn dry_run_g_script_post(&mut self) {
        let src = self.script_post_src.clone();
        let words = vec![0_u16; self.script_max_words_input.trim().parse::<usize>().unwrap_or(64).clamp(1, 512)];
        self.dry_run_g_script_one("POST", &src, words);
    }

    fn dry_run_g_script_one(&mut self, label: &str, src: &str, words: Vec<u16>) {
        self.script_output_open = true;
        let mut out = DryRunOutput::default();
        if let Err(e) = dry_run_one(label, src, &words, &mut out) {
            self.script_err = Some(e);
            return;
        }
        self.script_dry_run_output = out.summary.join("\n");
        self.script_print_log = if out.print_lines.is_empty() {
            String::new()
        } else {
            out.print_lines.join("\n")
        };
        self.script_regs_out = out.regs;
        self.script_regs_out.sort_by_key(|(reg_id, _)| *reg_id);
        self.script_emits_out = out.emits;
        self.script_output_tab = if !self.script_regs_out.is_empty() {
            1
        } else if !self.script_emits_out.is_empty() {
            2
        } else {
            0
        };
        self.script_status = Some(format!("{label}: тестовый запуск выполнен"));
        self.script_err = None;
    }

    pub(crate) fn clear_script_run_output(&mut self) {
        self.script_dry_run_output.clear();
        self.script_print_log.clear();
        self.script_regs_out.clear();
        self.script_emits_out.clear();
        self.script_output_tab = 0;
    }
}

#[derive(Default)]
struct DryRunOutput {
    summary: Vec<String>,
    print_lines: Vec<String>,
    regs: Vec<(i32, f64)>,
    emits: Vec<(f64, i32, f64)>,
}

fn format_keys(keys: &[i32]) -> String {
    if keys.is_empty() {
        return "-".to_string();
    }
    let mut out: Vec<String> = keys.iter().take(12).map(|v| v.to_string()).collect();
    if keys.len() > 12 {
        out.push(format!("+{}", keys.len() - 12));
    }
    out.join(",")
}

fn dry_run_one(label: &str, src: &str, words: &[u16], out: &mut DryRunOutput) -> Result<(), String> {
    let trimmed = src.trim();
    if trimmed.is_empty() {
        out.summary.push(format!("{label}: пустой"));
        return Ok(());
    }
    let script = script_dsl::Script::parse(trimmed).map_err(|e| format!("{label}: ошибка разбора: {e}"))?;
    out.summary.push(format!("{label}: разбор ok; rv keys: {}", format_keys(script.used_rv_keys())));
    let prints = RefCell::new(Vec::new());
    let emits = RefCell::new(Vec::new());
    let result = script
        .eval_result(
            words,
            true,
            &|_| 0.0,
            &|_, _| 0.0,
            Some(&|msg| prints.borrow_mut().push(msg.to_string())),
            Some(&|ts, reg_id, value| {
                emits
                    .borrow_mut()
                    .push((ts, reg_id, value));
            }),
            100000,
        )
        .map_err(|e| format!("{label}: ошибка выполнения: {e}"))?;
    let mut regs: Vec<_> = result.regs.into_iter().collect();
    regs.sort_by_key(|(reg_id, _)| *reg_id);
    out.summary.push(format!("{label}: regs {}", regs.len()));
    for (reg_id, value) in regs {
        out.regs.push((reg_id, value));
    }
    let prints = prints.into_inner();
    if !prints.is_empty() {
        out.summary.push(format!("{label}: print {}", prints.len()));
        for msg in prints {
            out.print_lines.push(format!("{label}: {msg}"));
        }
    }
    let emits = emits.into_inner();
    if !emits.is_empty() {
        out.summary.push(format!("{label}: emit {}", emits.len()));
        for (ts, reg_id, value) in emits {
            out.emits.push((ts, reg_id, value));
        }
    }
    Ok(())
}
