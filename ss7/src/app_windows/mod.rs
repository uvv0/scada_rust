//! Окна КПЗ: типы результатов воркеров и хелперы (каждое окно — отдельный файл).

mod create_from_template;
mod delete;
mod helpers;
mod load_window;
mod reload_kp_template_windows;
mod reload_windows;
mod upsert;

pub use create_from_template::CreateWindowFromTemplateWorkerResult;
pub use delete::DeleteWindowWorkerResult;
pub use helpers::{
    default_window_description, next_available_window_code, parse_binding_value_from_words_static,
};
pub use load_window::LoadWindowWorkerResult;
pub use reload_kp_template_windows::{
    ReloadKpBindingTemplateWindowsWorkerResult, ReloadKpTemplateWindowsWorkerResult,
};
pub use reload_windows::ReloadWindowsWorkerResult;
pub use upsert::UpsertWindowWorkerResult;
