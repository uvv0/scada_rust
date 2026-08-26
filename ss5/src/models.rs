#[derive(Clone, Debug)]
pub struct KpzRow {
    pub id: i32,
    pub name: String,
    pub rtu: i32,
    pub obj: i32,
    pub modem: Option<i32>,
    pub max_pkt_len: Option<i32>,
    pub start: i32,
    pub grups: Vec<u8>,
    pub t_a: String,
    pub t_script: String,
    pub en_post: bool,
}

#[derive(Clone, Debug)]
pub struct ObjRow {
    pub id: i32,
    pub name: String,
    pub ip_raw: Option<String>,
    pub ip: Option<i32>,
    pub port: Option<i32>,
    pub kanal: Option<i32>,
    pub speed: Option<i32>,
    pub stop: Option<i32>,
    pub parit: Option<i32>,
    pub bit: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct DictItemRow {
    pub id: i32,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct GroupRow {
    pub id: i32,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct RegRow {
    pub id: i32,
    pub name: String,
    pub mb: i32,
    pub tip: i32,
    pub bits: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct KpzIoRow {
    pub id: i32,
    pub name: String,
    pub mb: i32,
    pub tip: i32,
    pub bits: Option<i32>,
    pub reg_val: Option<f64>,
    pub last_val: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct RegEditRow {
    pub id: i32,
    pub name: String,
    pub mb: i32,
    pub n_mb: Option<i32>,
    pub tip: i32,
    pub bits: Option<i32>,
    pub grup: Option<i32>,
    pub a_en: bool,
    pub a_no_write: i32,
}

#[derive(Clone, Debug)]
pub struct ArxPointRow {
    pub ts_unix: i64,
    pub val_num: f64,
}

#[derive(Clone, Debug)]
pub struct ArxSeriesRow {
    pub reg_id: i32,
    pub points: Vec<ArxPointRow>,
}

#[derive(Clone, Debug)]
pub struct ArxStateRow {
    pub kpz_id: i32,
    pub arx_id: i32,
    pub last_ind: i32,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ArxValViewRow {
    pub id: i64,
    pub kpz_id: i32,
    pub reg_id: i32,
    pub tip: i16,
    pub val_num: Option<f64>,
    pub ts_unix: i64,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct PollLogRow {
    pub ts: String,
    pub kpz_id: Option<i32>,
    pub kind: String,
    pub msg: String,
}

#[derive(Clone, Debug)]
pub struct ElamRow {
    pub id: i64,
    pub ts: String,
    pub kpz_id: i32,
    pub group_id: Option<i32>,
    pub status: String,
    pub duration_ms: Option<i32>,
    pub func: Option<i32>,
    pub addr_human: Option<i32>,
    pub count_words: Option<i32>,
    pub req: Vec<u8>,
    pub resp: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct GScriptRow {
    pub grup: i32,
    pub elam: i32,
    pub max_words: i32,
    pub max_k: i32,
    pub pre_src: String,
    pub post_src: String,
    pub en: bool,
    pub ver: i32,
}

#[derive(Clone, Debug)]
pub struct GScriptTemplateRow {
    pub id: i64,
    pub name: String,
    pub pre_src: String,
    pub post_src: String,
    pub elam: i32,
    pub max_words: i32,
    pub max_k: i32,
    pub en: bool,
    pub ver: i32,
}

#[derive(Clone, Debug)]
pub struct AlarmRuleRow {
    pub id: i64,
    pub kpz_id: i32,
    pub reg_id: i32,
    pub enabled: bool,
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
    pub chat_id: Option<String>,
    pub tg_on_on: bool,
    pub tg_on_off: bool,
    pub tg_thr_main: bool,
    pub tg_thr_lvl1: bool,
}

#[derive(Clone, Debug)]
pub struct AlarmStateRow {
    pub rule_id: i64,
    pub kpz_id: i32,
    pub reg_id: i32,
    pub active: bool,
    pub active_since: Option<String>,
    pub last_value: Option<f64>,
    #[allow(dead_code)]
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct AlarmEventRow {
    #[allow(dead_code)]
    pub id: i64,
    pub ts: String,
    pub kpz_id: i32,
    pub reg_id: i32,
    pub rule_id: i64,
    pub event: String,
    pub value: Option<f64>,
    #[allow(dead_code)]
    pub set_lo: Option<f64>,
    #[allow(dead_code)]
    pub set_hi: Option<f64>,
    #[allow(dead_code)]
    pub severity: i16,
    #[allow(dead_code)]
    pub code: Option<String>,
    #[allow(dead_code)]
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SchedulerRuntimeCfgRow {
    #[allow(dead_code)]
    pub id: i64,
    pub no_response_failures: i32,
    pub no_response_backoff_sec: i64,
    pub metrics_p95_warn_ms: i64,
    pub metrics_p95_crit_ms: i64,
    pub modbus_a_timeout_ms: i64,
    pub modbus_script_timeout_ms: i64,
}

#[derive(Clone, Debug)]
pub struct UiKpzWindowRow {
    pub id: i64,
    #[allow(dead_code)]
    pub kpz_id: i32,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    #[allow(dead_code)]
    pub is_active: bool,
}

#[derive(Clone, Debug)]
pub struct UiWindowGroupRow {
    pub group_id: i32,
    pub pos: i32,
}

#[derive(Clone, Debug)]
pub struct UiWindowBindingRow {
    pub reg_id: i32,
    pub pos: i32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub visible: bool,
    pub writable: bool,
    pub label_override: Option<String>,
    pub unit: Option<String>,
    pub fmt: Option<String>,
    pub reg_name: String,
    pub reg_mb: i32,
    pub reg_tip: i32,
    pub reg_n_mb: i32,
    pub reg_bits: Option<i32>,
}
