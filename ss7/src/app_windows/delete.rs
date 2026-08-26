//! Результат воркера: удаление окна.

pub struct DeleteWindowWorkerResult {
    pub window_id: i64,
    pub err: Option<String>,
}
