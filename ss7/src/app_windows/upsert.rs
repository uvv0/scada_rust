//! Результат воркера: сохранение (create/update) окна.

pub struct UpsertWindowWorkerResult {
    pub window_id: i64,
    pub title: String,
    pub code: String,
    pub description: String,
    pub template_warning: Option<String>,
    pub err: Option<String>,
}
