//! Результат воркера: перезагрузка списка окон КПЗ.

use crate::models::UiKpzWindowRow;

pub struct ReloadWindowsWorkerResult {
    pub rows: Vec<UiKpzWindowRow>,
    pub status: Option<String>,
    pub err: Option<String>,
}
