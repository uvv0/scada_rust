//! Результаты воркеров: перезагрузка окон КП-шаблона и окон шаблона привязок.

use crate::models::UiKpTemplateWindowRow;

pub struct ReloadKpTemplateWindowsWorkerResult {
    pub rows: Vec<UiKpTemplateWindowRow>,
    pub status: Option<String>,
    pub err: Option<String>,
}

pub struct ReloadKpBindingTemplateWindowsWorkerResult {
    pub rows: Vec<UiKpTemplateWindowRow>,
    pub status: Option<String>,
    pub err: Option<String>,
}
