//! Результат воркера: создание окна из шаблона.

pub struct CreateWindowFromTemplateWorkerResult {
    pub template_id: i64,
    pub window_id: i64,
    pub window_code: String,
    pub window_title: String,
    pub window_description: String,
    pub err: Option<String>,
}
