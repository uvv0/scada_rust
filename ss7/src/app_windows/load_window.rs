//! Результат воркера: загрузка окна (метаданные + привязки).

use std::collections::{BTreeMap, BTreeSet};

use crate::models::{AlarmRuleRow, UiWindowBindingRow};

pub struct LoadWindowWorkerResult {
    pub window_id: i64,
    pub window_code: String,
    pub window_title: String,
    pub window_description: String,
    pub template_code: String,
    pub template_title: String,
    pub groups_selected: BTreeSet<i32>,
    pub bindings: Vec<UiWindowBindingRow>,
    pub alarm_rules_by_reg: BTreeMap<i32, Vec<AlarmRuleRow>>,
    pub err: Option<String>,
}
