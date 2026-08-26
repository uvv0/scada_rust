use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[allow(dead_code)]
pub struct WebUserAuthRow {
    pub id: i64,
    pub login: String,
    pub password_salt: String,
    pub password_hash: String,
    pub role: String,
    pub enabled: bool,
    pub kpz_from: Option<i32>,
    pub kpz_to: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebSessionUserDto {
    pub user_id: i64,
    pub login: String,
    pub role: String,
    pub kpz_from: Option<i32>,
    pub kpz_to: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub login: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResultDto {
    pub ok: bool,
    pub login: Option<String>,
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csrf_token: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct KpzDto {
    pub id: i32,
    pub name: String,
    pub start: i32,
}

#[derive(Debug, Serialize)]
pub struct GroupDto {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct RegDto {
    pub id: i32,
    pub name: String,
    pub mb: i32,
    pub tip: i32,
    pub bits: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct KpzQuery {
    pub kpz_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct GroupQuery {
    pub group_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct ArxSeriesQuery {
    pub kpz_id: i32,
    pub reg_ids: String,
    pub limit: Option<i64>,
    pub window_sec: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UiWindowsQuery {
    pub kpz_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct UiBindingsQuery {
    pub window_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct LiveValuesQuery {
    pub kpz_id: i32,
    pub reg_ids: String,
}

#[derive(Debug, Deserialize)]
pub struct TuWriteQuery {
    pub kpz_id: i32,
    pub reg_id: i32,
    pub on: i32,
}

#[derive(Debug, Serialize)]
pub struct TuWriteResultDto {
    pub ok: bool,
    pub req_hex: Option<String>,
    pub resp_hex: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WriteValueQuery {
    pub kpz_id: i32,
    pub reg_id: i32,
    pub val: f64,
}

#[derive(Debug, Serialize)]
pub struct WriteValueResultDto {
    pub ok: bool,
    pub req_hex: Option<String>,
    pub resp_hex: Option<String>,
    pub error: Option<String>,
    pub mb_addr: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct UiWindowDto {
    pub id: i64,
    pub code: String,
    pub title: String,
    pub is_template: bool,
}

#[derive(Debug, Serialize)]
pub struct UiBindingDto {
    pub reg_id: i32,
    pub is_text: bool,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub visible: bool,
    pub writable: bool,
    pub reg_name: String,
    pub reg_mb: i32,
    pub reg_n_mb: i32,
    pub reg_tip: i32,
    pub reg_bits: Option<i32>,
    pub label_override: Option<String>,
    pub unit: Option<String>,
    pub fmt: Option<String>,
    pub scale_max: Option<f64>,
    pub component_kind: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LiveValueDto {
    pub reg_id: i32,
    pub val_num: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct LiveValueRealDto {
    pub reg_id: i32,
    pub val_num: Option<f64>,
    pub io_ip: Option<String>,
    pub io_modem: Option<u16>,
    pub req_hex: Option<String>,
    pub resp_hex: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RegIoDto {
    pub id: i32,
    pub mb: i32,
    pub n_mb_id: i32,
    pub tip: i32,
    pub bits: Option<i32>,
}

#[derive(Debug)]
pub struct IoConnDto {
    pub ip: String,
    pub port: u16,
    pub rtu: u16,
    pub modem: u16,
    pub kan: u8,
    pub speed: u8,
    pub stop: u8,
    pub par: u8,
    pub data: u8,
    pub max_pkt_len: usize,
}

#[derive(Debug, Serialize)]
pub struct AlarmRulePreviewDto {
    pub reg_id: i32,
    pub set_lo: Option<f64>,
    pub set_hi: Option<f64>,
    pub set_lo_1: Option<f64>,
    pub set_hi_1: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct ArxPointDto {
    pub ts_unix: i64,
    pub val_num: f64,
}

#[derive(Debug, Serialize)]
pub struct ArxSeriesDto {
    pub reg_id: i32,
    pub points: Vec<ArxPointDto>,
}

#[derive(Debug, Serialize)]
pub struct WebActionDto {
    pub id: i64,
    pub user_id: i64,
    pub login: String,
    pub action: String,
    pub detail: String,
    pub kpz_id: Option<i32>,
    pub reg_id: Option<i32>,
    pub created_at: String,
}
