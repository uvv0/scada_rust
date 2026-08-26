//! Общие структуры данных доменной модели (KPZ/OBJ/ARX/Alarm/Bindings).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Строка `kpz` с параметрами устройства и расписанием его опроса.
pub struct KpzRow {
    pub id: i32,
    pub name: Option<String>,
    pub rtu: i32,
    pub obj: i32,
    pub modem: i32,
    pub grups: Vec<u8>,
    pub max_pkt_len: i32,
    pub start: i32,
    pub t_a: i32,
    pub t_script: i32,
    pub en_post: bool,
}

#[derive(Debug, Clone)]
/// Строка `obj` с сетевыми/канальными параметрами подключения.
pub struct ObjRow {
    pub id: i32,
    #[allow(dead_code)]
    pub name: Option<String>,
    pub ip: Option<String>,
    pub port: Option<String>,
    pub kanal: Option<i32>,
    pub speed: Option<i32>,
    pub stop: Option<i32>,
    pub parit: Option<i32>,
    pub bit: Option<i32>,
}

#[derive(Debug, Clone)]
/// Нормализованные реквизиты соединения, уже готовые для Modbus/UDP-обмена.
pub struct ConnInfo {
    pub kpz_id: i32,
    pub obj_id: i32,
    pub ip: String,
    pub port: u16,
    pub rtu: i32,
    pub modem: i32,
    pub max_pkt_len: i32,
}

#[derive(Debug, Clone)]
/// Конфигурация скрипта группы (`g_script`) c PRE/POST исходниками и лимитами.
pub struct GScriptRow {
    pub grup: i32,
    pub pre_src: Option<String>,
    pub post_src: Option<String>,
    pub max_k: Option<i32>,
    pub max_words: Option<i32>,
    pub en: Option<bool>,
    pub ver: Option<i32>,
}

#[derive(Debug, Clone)]
/// Строка вставки в `arx_val` для сохранения рассчитанного значения регистра.
pub struct ArxValRow {
    pub kpz_id: i32,
    pub reg_id: i32,
    pub ts_unix: i64,
    pub tip: i32,
    pub val_num: f64,
    pub val_raw: Vec<u8>,
}

#[derive(Debug, Clone)]
/// Правило аварийной сигнализации, загруженное из `alarm_rule`.
pub struct AlarmRule {
    pub id: i64,
    pub kpz_id: i32,
    pub reg_id: i32,
    pub cmp: String,
    pub set_lo: Option<f64>,
    pub set_hi: Option<f64>,
    pub set_lo_1: Option<f64>,
    pub set_hi_1: Option<f64>,
    pub hysteresis: f64,
    pub on_delay_sec: i32,
    pub off_delay_sec: i32,
    pub severity: i16,
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
/// Привязка логических индексов скрипта к реальным регистрам/адресам для конкретного КПЗ.
pub struct ScriptBindingRow {
    pub kpz_id: i32,
    pub grup: i32,
    pub logical: i32,
    pub reg_id: Option<i32>,
    pub addr: Option<i32>,
}

#[derive(Debug, Clone)]
/// Telegram-настройки уведомления для одного alarm-правила.
pub struct AlarmNotifyRoute {
    pub rule_id: i64,
    pub chat_id: String,
    pub on_on: bool,
    pub on_off: bool,
    pub thr_main: bool,
    pub thr_lvl1: bool,
}
