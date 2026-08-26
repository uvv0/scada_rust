use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU8, Ordering};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path as AxumPath, Query, Request, State, ws::{Message, WebSocket, WebSocketUpgrade}};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio_postgres::Client;

use crate::db::{
    create_web_session, delete_web_session, insert_web_action, load_alarm_rules_for_kpz,
    load_arx_series, load_groups_for_kpz, load_io_conn_for_kpz, load_kpz, load_live_values,
    load_n_mb_dict, load_recent_web_actions, load_regs_for_group, load_regs_io_by_ids,
    load_ui_bindings, load_ui_windows_for_kpz, load_web_session_user, load_web_user_by_login,
    parse_reg_ids,
};
use crate::modbus;
use crate::models::{
    ArxSeriesDto, ArxSeriesQuery, GroupQuery, IoConnDto, KpzQuery, LiveValueDto, LiveValueRealDto,
    LiveValuesQuery, LoginRequest, LoginResultDto, RegIoDto, TuWriteQuery, TuWriteResultDto,
    UiBindingsQuery, UiWindowsQuery, WriteValueQuery, WriteValueResultDto,
};
use crate::web::{index_html, login_html};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Client>,
}

const APP_JS: &str = include_str!("../assets/app.js");
const API_JS: &str = include_str!("../assets/api.js");
const APP_CSS: &str = include_str!("../assets/app.css");
const CHART_CONTROLLER_JS: &str = include_str!("../assets/chart_controller.js");
const META_STATE_JS: &str = include_str!("../assets/meta_state.js");
const PREVIEW_SCALE_JS: &str = include_str!("../assets/preview_scale.js");
const PREVIEW_POLL_JS: &str = include_str!("../assets/preview_poll.js");
const PREVIEW_MODALS_JS: &str = include_str!("../assets/preview_modals.js");
const PREVIEW_SCENE_JS: &str = include_str!("../assets/preview_scene.js");
const SESSION_COOKIE: &str = "ss6_session";
const CSRF_COOKIE: &str = "ss6_csrf";
static TOKEN_SEQ: AtomicU16 = AtomicU16::new(1);

static RATE_LIMIT: LazyLock<std::sync::Mutex<HashMap<String, Vec<Instant>>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
const RATE_MAX: usize = 10;
const RATE_WINDOW: Duration = Duration::from_secs(5);

fn rate_limit(ip: &str) -> bool {
    let now = Instant::now();
    let mut map = RATE_LIMIT.lock().unwrap();
    let entries = map.entry(ip.to_string()).or_default();
    entries.retain(|t| now.duration_since(*t) < RATE_WINDOW);
    if entries.len() >= RATE_MAX {
        return false;
    }
    entries.push(now);
    true
}

#[derive(Serialize)]
struct ApiErrorBody {
    code: &'static str,
    message: String,
}

fn api_error(status: StatusCode, code: &'static str, message: impl Into<String>) -> Response {
    (status, Json(ApiErrorBody { code, message: message.into() })).into_response()
}

pub fn hash_password_legacy(password_salt: &str, password: &str) -> String {
    let mut h = Sha256::new();
    h.update(password_salt.as_bytes());
    h.update(b":");
    h.update(password.as_bytes());
    let digest = h.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

pub fn make_password_salt() -> String {
    SaltString::generate(&mut OsRng).to_string()
}

pub fn hash_password_argon2(password_salt: &str, password: &str) -> anyhow::Result<String> {
    let salt = SaltString::from_b64(password_salt)
        .map_err(|e| anyhow::anyhow!("invalid password salt: {e}"))?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("failed to hash password: {e}"))?;
    Ok(hash.to_string())
}

pub fn verify_password(password_salt: &str, password_hash: &str, password: &str) -> bool {
    if password_hash.starts_with("$argon2") {
        let Ok(parsed) = PasswordHash::new(password_hash) else {
            return false;
        };
        return Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok();
    }
    password_hash == hash_password_legacy(password_salt, password)
}

pub fn make_secret_token(seed: &str) -> String {
    let mut h = Sha256::new();
    h.update(seed.as_bytes());
    h.update(format!(
        ":{}:{}:{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default(),
        TOKEN_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let digest = h.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let mut it = part.trim().splitn(2, '=');
        let key = it.next()?.trim();
        let val = it.next()?.trim();
        if key == name && !val.is_empty() {
            return Some(val.to_string());
        }
    }
    None
}

fn session_cookie_header(token: &str) -> String {
    format!("{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000")
}

fn ws_token_cookie_header(token: &str) -> String {
    format!("ss6_ws_token={token}; Path=/; SameSite=Lax; Max-Age=2592000")
}

fn clear_session_cookie_header() -> String {
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

fn csrf_cookie_header(token: &str) -> String {
    format!("{CSRF_COOKIE}={token}; Path=/; SameSite=Lax; Max-Age=2592000")
}

fn clear_csrf_cookie_header() -> String {
    format!("{CSRF_COOKIE}=; Path=/; SameSite=Lax; Max-Age=0")
}

fn read_csrf_token(headers: &HeaderMap) -> Option<String> {
    read_cookie(headers, CSRF_COOKIE)
}

fn validate_csrf(headers: &HeaderMap) -> Result<(), Response> {
    let cookie_val = read_csrf_token(headers);
    let header_val = headers
        .get("x-csrf-token")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string());
    match (cookie_val, header_val) {
        (Some(c), Some(h)) if c == h && !c.is_empty() => Ok(()),
        _ => Err(api_error(
            StatusCode::FORBIDDEN,
            "CSRF_INVALID",
            "missing or invalid CSRF token",
        )),
    }
}

fn role_rank(role: &str) -> i32 {
    match role.trim().to_ascii_lowercase().as_str() {
        "admin" => 3,
        "operator" => 2,
        "viewer" => 1,
        _ => 0,
    }
}

fn required_role_rank(method: &Method, path: &str) -> i32 {
    if path == "/" {
        return 1;
    }
    if path.starts_with("/api/") {
        return match (method, path) {
            (&Method::POST, "/api/tu_write") => 2,
            (&Method::POST, "/api/write_value") => 2,
            _ => 1,
        };
    }
    1
}

fn kpz_allowed(user: &crate::models::WebSessionUserDto, kpz_id: i32) -> bool {
    if let Some(from) = user.kpz_from {
        if kpz_id < from {
            return false;
        }
    }
    if let Some(to) = user.kpz_to {
        if kpz_id > to {
            return false;
        }
    }
    true
}

async fn session_user_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::models::WebSessionUserDto, Response> {
    let Some(token) = read_cookie(headers, SESSION_COOKIE) else {
        return Err(api_error(StatusCode::UNAUTHORIZED, "AUTH_REQUIRED", "login required"));
    };
    match load_web_session_user(&state.db, &token).await {
        Ok(Some(user)) => Ok(user),
        Ok(None) => Err(api_error(StatusCode::UNAUTHORIZED, "AUTH_REQUIRED", "login required")),
        Err(e) => Err(api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "AUTH_SESSION_FAILED",
            e.to_string(),
        )),
    }
}

pub async fn require_auth(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    let token = read_cookie(req.headers(), SESSION_COOKIE);
    if let Some(token) = token {
        match load_web_session_user(&state.db, &token).await {
            Ok(Some(user)) => {
                let need = required_role_rank(&method, &path);
                if role_rank(&user.role) >= need {
                    return next.run(req).await;
                }
                return api_error(StatusCode::FORBIDDEN, "AUTH_FORBIDDEN", format!("role '{}' is not allowed for {}", user.role, path));
            }
            Ok(None) => {}
            Err(e) => {
                return api_error(StatusCode::INTERNAL_SERVER_ERROR, "AUTH_SESSION_FAILED", e.to_string());
            }
        }
    }
    if path.starts_with("/api/") {
        return api_error(StatusCode::UNAUTHORIZED, "AUTH_REQUIRED", "login required");
    }
    (
        StatusCode::SEE_OTHER,
        [(header::LOCATION, "/login"), (header::CACHE_CONTROL, "no-store")],
    )
        .into_response()
}

pub async fn login_page() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        login_html(),
    )
}

pub async fn api_login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let login = body.login.trim();
    let password = body.password.trim();
    if login.is_empty() || password.is_empty() {
        return api_error(StatusCode::BAD_REQUEST, "BAD_LOGIN", "login and password required");
    }
    let Some(user) = (match load_web_user_by_login(&state.db, login).await {
        Ok(v) => v,
        Err(e) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, "AUTH_LOOKUP_FAILED", e.to_string()),
    }) else {
        return api_error(StatusCode::UNAUTHORIZED, "AUTH_FAILED", "invalid login or password");
    };
    if !user.enabled || !verify_password(&user.password_salt, &user.password_hash, password) {
        return api_error(StatusCode::UNAUTHORIZED, "AUTH_FAILED", "invalid login or password");
    }
    let session_token = make_secret_token(&format!("{}:{}", user.login, user.id));
    if let Err(e) = create_web_session(&state.db, user.id, &session_token).await {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, "AUTH_SESSION_CREATE_FAILED", e.to_string());
    }
    let csrf_token = make_secret_token(&format!("csrf:{}:{}", user.login, user.id));
    let mut resp = Json(LoginResultDto {
        ok: true,
        login: Some(user.login),
        role: Some(user.role),
        csrf_token: Some(csrf_token.clone()),
        error: None,
    })
    .into_response();
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if let Ok(v) = header::HeaderValue::from_str(&session_cookie_header(&session_token)) {
        resp.headers_mut().insert(header::SET_COOKIE, v);
    }
    if let Ok(v) = header::HeaderValue::from_str(&ws_token_cookie_header(&session_token)) {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    if let Ok(v) = header::HeaderValue::from_str(&csrf_cookie_header(&csrf_token)) {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    resp
}

pub async fn api_logout(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(token) = read_cookie(&headers, SESSION_COOKIE) {
        if let Err(resp) = validate_csrf(&headers) {
            return resp;
        }
        let _ = delete_web_session(&state.db, &token).await;
    }
    let mut resp = Json(LoginResultDto {
        ok: true,
        login: None,
        role: None,
        csrf_token: None,
        error: None,
    })
    .into_response();
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if let Ok(v) = header::HeaderValue::from_str(&clear_session_cookie_header()) {
        resp.headers_mut().insert(header::SET_COOKIE, v);
    }
    if let Ok(v) = header::HeaderValue::from_str("ss6_ws_token=; Path=/; SameSite=Lax; Max-Age=0") {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    if let Ok(v) = header::HeaderValue::from_str(&clear_csrf_cookie_header()) {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    resp
}

pub async fn index() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        index_html(),
    )
}

pub async fn static_app_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        APP_JS,
    )
}

pub async fn static_app_css() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        APP_CSS,
    )
}

pub async fn static_api_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        API_JS,
    )
}

pub async fn static_chart_controller_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        CHART_CONTROLLER_JS,
    )
}

pub async fn static_meta_state_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        META_STATE_JS,
    )
}

pub async fn static_preview_scale_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        PREVIEW_SCALE_JS,
    )
}

pub async fn static_preview_poll_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        PREVIEW_POLL_JS,
    )
}

pub async fn static_preview_modals_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        PREVIEW_MODALS_JS,
    )
}

pub async fn static_preview_scene_js() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
        ],
        PREVIEW_SCENE_JS,
    )
}

pub async fn api_kpz(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let user = match session_user_from_headers(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    match load_kpz(&state.db).await {
        Ok(v) => Json(v.into_iter().filter(|x| kpz_allowed(&user, x.id)).collect::<Vec<_>>()).into_response(),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string()),
    }
}

pub async fn api_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<KpzQuery>,
) -> impl IntoResponse {
    let user = match session_user_from_headers(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if !kpz_allowed(&user, q.kpz_id) {
        return api_error(StatusCode::FORBIDDEN, "KPZ_FORBIDDEN", "kpz not allowed");
    }
    match load_groups_for_kpz(&state.db, q.kpz_id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string()),
    }
}

pub async fn api_regs(
    State(state): State<AppState>,
    Query(q): Query<GroupQuery>,
) -> impl IntoResponse {
    match load_regs_for_group(&state.db, q.group_id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string()),
    }
}

pub async fn api_arx_series(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ArxSeriesQuery>,
) -> impl IntoResponse {
    let user = match session_user_from_headers(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if !kpz_allowed(&user, q.kpz_id) {
        return api_error(StatusCode::FORBIDDEN, "KPZ_FORBIDDEN", "kpz not allowed");
    }
    let ids = parse_reg_ids(&q.reg_ids);
    if ids.is_empty() {
        return Json(Vec::<ArxSeriesDto>::new()).into_response();
    }
    let limit = q.limit.unwrap_or(1500).clamp(100, 10000);
    let window_sec = q.window_sec.unwrap_or(86400).clamp(60, 60 * 60 * 24 * 30);
    match load_arx_series(&state.db, q.kpz_id, &ids, limit, window_sec).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string()),
    }
}

pub async fn api_ui_windows(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<UiWindowsQuery>,
) -> impl IntoResponse {
    let user = match session_user_from_headers(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if !kpz_allowed(&user, q.kpz_id) {
        return api_error(StatusCode::FORBIDDEN, "KPZ_FORBIDDEN", "kpz not allowed");
    }
    match load_ui_windows_for_kpz(&state.db, q.kpz_id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string()),
    }
}

pub async fn api_ui_bindings(
    State(state): State<AppState>,
    Query(q): Query<UiBindingsQuery>,
) -> impl IntoResponse {
    match load_ui_bindings(&state.db, q.window_id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string()),
    }
}

pub async fn api_live_values(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<LiveValuesQuery>,
) -> impl IntoResponse {
    let user = match session_user_from_headers(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if !kpz_allowed(&user, q.kpz_id) {
        return api_error(StatusCode::FORBIDDEN, "KPZ_FORBIDDEN", "kpz not allowed");
    }
    let ids = parse_reg_ids(&q.reg_ids);
    if ids.is_empty() {
        return Json(Vec::<LiveValueDto>::new()).into_response();
    }
    match load_live_values(&state.db, q.kpz_id, &ids).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string()),
    }
}

pub async fn api_tu_write(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(q): Json<TuWriteQuery>,
) -> impl IntoResponse {
    if let Err(resp) = validate_csrf(&headers) {
        return resp;
    }
    let ip = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()).unwrap_or("unknown");
    if !rate_limit(ip) {
        return api_error(StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED", "too many requests, try again later");
    }
    let user = match session_user_from_headers(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if !kpz_allowed(&user, q.kpz_id) {
        return api_error(StatusCode::FORBIDDEN, "KPZ_FORBIDDEN", "kpz not allowed");
    }
    let Some(conn) = (match load_io_conn_for_kpz(&state.db, q.kpz_id).await {
        Ok(v) => v,
        Err(e) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, "IO_CONFIG_ERROR", e.to_string()),
    }) else {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, "IO_CONFIG_MISSING", "kpz io config not found");
    };

    let regs = match load_regs_io_by_ids(&state.db, &[q.reg_id]).await {
        Ok(v) => v,
        Err(e) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string()),
    };
    let Some(r) = regs.into_iter().next() else {
        return api_error(StatusCode::BAD_REQUEST, "REG_NOT_FOUND", "reg not found");
    };
    if !(r.n_mb_id == 1 || r.tip == 1) {
        return api_error(StatusCode::BAD_REQUEST, "NOT_TU_REG", "reg is not TU (need n_mb=1 or tip=1)");
    }

    let data = if q.on != 0 { [0xFF, 0x00] } else { [0x00, 0x00] };
    let mb = match modbus::sout_mb_only(conn.rtu, 5, r.mb, 1, Some(&data)) {
        Ok(v) => v,
        Err(e) => return api_error(StatusCode::BAD_REQUEST, "MODBUS_BUILD_ERROR", e),
    };
    match send_mb_chunk_over_udp(&conn, &[mb], Duration::from_millis(5000)) {
        Ok((tx, resp)) => {
            let req_hex = hex_join(&tx);
            let resp_hex = hex_join(&resp);
            let mbf = modbus::extract_modbus_frame(&resp).unwrap_or(&[]);
            if mbf.len() < 6 {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(ApiErrorBody { code: "MODBUS_SHORT_RESPONSE", message: format!("short write response tx={} rx={}", req_hex, resp_hex) }),
                )
                    .into_response();
            }
            let ulen = if mbf[0] >= 0xF8 { 2 } else { 1 };
            let fi = ulen;
            if mbf.len() <= fi + 4 {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(ApiErrorBody { code: "MODBUS_BAD_RESPONSE", message: format!("bad write response tx={} rx={}", req_hex, resp_hex) }),
                )
                    .into_response();
            }
            let func = mbf[fi];
            if (func & 0x80) != 0 {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(ApiErrorBody { code: "MODBUS_EXCEPTION", message: format!("modbus exception func={} tx={} rx={}", func, req_hex, resp_hex) }),
                )
                    .into_response();
            }
            if func != 5 {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(ApiErrorBody { code: "MODBUS_UNEXPECTED_FUNC", message: format!("unexpected func={}, expected=5 tx={} rx={}", func, req_hex, resp_hex) }),
                )
                    .into_response();
            }
            let resp = (
                StatusCode::OK,
                Json(TuWriteResultDto {
                    ok: true,
                    req_hex: Some(req_hex.clone()),
                    resp_hex: Some(resp_hex.clone()),
                    error: None,
                }),
            )
                .into_response();
            let db = state.db.clone();
            let uid = user.user_id;
            let kz = q.kpz_id;
            let rid = q.reg_id;
            let on = q.on;
            let detail = format!("TU {} reg={} kpz={} tx={} rx={}", if on != 0 { "ON" } else { "OFF" }, rid, kz, req_hex, resp_hex);
            tokio::spawn(async move {
                let _ = insert_web_action(&db, uid, "tu_write", &detail, Some(kz), Some(rid)).await;
            });
            resp
        }
        Err(e) => api_error(StatusCode::BAD_GATEWAY, "UDP_IO_ERROR", e),
    }
}

pub async fn api_write_value(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(q): Json<WriteValueQuery>,
) -> impl IntoResponse {
    if let Err(resp) = validate_csrf(&headers) {
        return resp;
    }
    let ip = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()).unwrap_or("unknown");
    if !rate_limit(ip) {
        return api_error(StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED", "too many requests, try again later");
    }
    let user = match session_user_from_headers(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if !kpz_allowed(&user, q.kpz_id) {
        return api_error(StatusCode::FORBIDDEN, "KPZ_FORBIDDEN", "kpz not allowed");
    }
    let Some(conn) = (match load_io_conn_for_kpz(&state.db, q.kpz_id).await {
        Ok(v) => v,
        Err(e) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, "IO_CONFIG_ERROR", e.to_string()),
    }) else {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, "IO_CONFIG_MISSING", "kpz io config not found");
    };

    let regs = match load_regs_io_by_ids(&state.db, &[q.reg_id]).await {
        Ok(v) => v,
        Err(e) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string()),
    };
    let Some(r) = regs.into_iter().next() else {
        return api_error(StatusCode::BAD_REQUEST, "REG_NOT_FOUND", "reg not found");
    };
    if r.n_mb_id == 1 || r.tip == 1 {
        return api_error(StatusCode::BAD_REQUEST, "TU_REG", "reg is TU command, use FC5 ON/OFF");
    }

    let send_write = |mb: Vec<u8>, expected_func: u8| -> Result<(String, String), String> {
        let (tx, resp) = send_mb_chunk_over_udp(&conn, &[mb.clone()], Duration::from_millis(5000))?;
        let req_hex = hex_join(&tx);
        let resp_hex = hex_join(&resp);
        let mbf = modbus::extract_modbus_frame(&resp).unwrap_or(&[]);
        if mbf.len() < 6 {
            return Err(format!("short write response tx={} rx={}", req_hex, resp_hex));
        }
        let ulen = if mbf[0] >= 0xF8 { 2 } else { 1 };
        let fi = ulen;
        if mbf.len() <= fi + 4 {
            return Err(format!("bad write response tx={} rx={}", req_hex, resp_hex));
        }
        let func = mbf[fi];
        if (func & 0x80) != 0 {
            return Err(format!(
                "modbus exception func={} tx={} rx={}",
                func, req_hex, resp_hex
            ));
        }
        if func != expected_func {
            return Err(format!(
                "unexpected func={}, expected={} tx={} rx={}",
                func, expected_func, req_hex, resp_hex
            ));
        }
        if mbf.len() >= fi + 4 {
            let resp_adr = ((mbf[fi + 1] as u16) << 8) | (mbf[fi + 2] as u16);
            let mb_modbus = &mb[ulen..];
            let req_adr = ((mb_modbus[1] as u16) << 8) | (mb_modbus[2] as u16);
            if resp_adr != req_adr {
                return Err(format!(
                    "address mismatch: resp_addr={} req_addr={} tx={} rx={}",
                    resp_adr, req_adr, req_hex, resp_hex
                ));
            }
        }
        Ok((req_hex, resp_hex))
    };

    if matches!(r.tip, 2 | 4 | 5) {
        let dat = match r.tip {
            5 => (q.val as f32).to_be_bytes().to_vec(),
            4 => (q.val.max(0.0) as u32).to_be_bytes().to_vec(),
            2 => (q.val as i32).to_be_bytes().to_vec(),
            _ => (q.val as i32).to_be_bytes().to_vec(),
        };
        let mb16 = match modbus::sout_mb_only(conn.rtu, 16, r.mb, 2, Some(&dat)) {
            Ok(v) => v,
            Err(e) => return api_error(StatusCode::BAD_REQUEST, "MODBUS_BUILD_ERROR", e),
        };
        return match send_write(mb16, 16) {
            Ok((req_hex, resp_hex)) => {
                let db = state.db.clone();
                let uid = user.user_id;
                let kz = q.kpz_id;
                let rid = q.reg_id;
                let val = q.val;
                let detail = format!("write_val reg={} val={} kpz={} tx={} rx={}", rid, val, kz, req_hex, resp_hex);
                tokio::spawn(async move {
                    let _ = insert_web_action(&db, uid, "write_value", &detail, Some(kz), Some(rid)).await;
                });
                (
                    StatusCode::OK,
                    Json(WriteValueResultDto {
                        ok: true,
                        req_hex: Some(req_hex),
                        resp_hex: Some(resp_hex),
                        error: None,
                        mb_addr: Some(r.mb),
                    }),
                )
                    .into_response()
            }
            Err(e) => api_error(StatusCode::BAD_GATEWAY, "MODBUS_WRITE_FAILED", e),
        };
    }

    let mut target_word = {
        let v = q.val;
        if v < 0.0 || v > 65535.0 {
            return api_error(StatusCode::BAD_REQUEST, "VALUE_OUT_OF_RANGE", "value must be between 0 and 65535 for 16-bit register");
        }
        v.round() as u16
    };
    if r.tip == 0 {
        if let Some(bit) = r.bits {
            if (0..=15).contains(&bit) {
                let n_mb_by_id = match load_n_mb_dict(&state.db).await {
                    Ok(v) => v,
                    Err(e) => {
                        return api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string())
                    }
                };
                let read_func = read_func_by_n_mb_id(r.n_mb_id, &n_mb_by_id);
                let read_mb = match modbus::sout_mb_only(conn.rtu, read_func, r.mb, 1, None) {
                    Ok(v) => v,
                    Err(e) => return api_error(StatusCode::BAD_REQUEST, "MODBUS_BUILD_ERROR", e),
                };
                let (_, read_resp) = match send_mb_chunk_over_udp(&conn, &[read_mb], Duration::from_millis(5000))
                {
                    Ok(v) => v,
                    Err(e) => return api_error(StatusCode::BAD_GATEWAY, "UDP_IO_ERROR", e),
                };
                let words = match parse_read_words_from_resp(&read_resp, read_func) {
                    Ok(v) => v,
                    Err(e) => return api_error(StatusCode::BAD_GATEWAY, "MODBUS_READ_FAILED", e),
                };
                let cur = words.first().copied().unwrap_or(0);
                let mask = 1u16 << bit;
                let on = q.val >= 0.5;
                target_word = if on { cur | mask } else { cur & !mask };
            }
        }
    }

    let dat = [((target_word >> 8) & 0xFF) as u8, (target_word & 0xFF) as u8];
    let mb6 = match modbus::sout_mb_only(conn.rtu, 6, r.mb, 1, Some(&dat)) {
        Ok(v) => v,
        Err(e) => return api_error(StatusCode::BAD_REQUEST, "MODBUS_BUILD_ERROR", e),
    };
    let mb16 = match modbus::sout_mb_only(conn.rtu, 16, r.mb, 1, Some(&dat)) {
        Ok(v) => v,
        Err(e) => return api_error(StatusCode::BAD_REQUEST, "MODBUS_BUILD_ERROR", e),
    };

    let mut last_err: Option<String> = None;
    for (mb, expected_func, mode) in [(mb6, 6u8, "FC6"), (mb16, 16u8, "FC16")] {
        match send_write(mb, expected_func) {
            Ok((req_hex, resp_hex)) => {
                let db = state.db.clone();
                let uid = user.user_id;
                let kz = q.kpz_id;
                let rid = q.reg_id;
                let val = q.val;
                let detail = format!("write_val reg={} val={} kpz={} tx={} rx={} mode={}", rid, val, kz, req_hex, resp_hex, mode);
                tokio::spawn(async move {
                    let _ = insert_web_action(&db, uid, "write_value", &detail, Some(kz), Some(rid)).await;
                });
                return (
                    StatusCode::OK,
                    Json(WriteValueResultDto {
                        ok: true,
                        req_hex: Some(req_hex),
                        resp_hex: Some(resp_hex),
                        error: Some(format!("write mode: {}", mode)),
                        mb_addr: Some(r.mb),
                    }),
                )
                    .into_response();
            }
            Err(e) => {
                last_err = Some(format!("{} failed: {}", mode, e));
            }
        }
    }

    api_error(
        StatusCode::BAD_GATEWAY,
        "MODBUS_WRITE_FAILED",
        last_err.unwrap_or_else(|| "write failed".to_string()),
    )
}

fn read_func_by_n_mb_id(n_mb_id: i32, n_mb_by_id: &HashMap<i32, String>) -> u8 {
    let name = n_mb_by_id
        .get(&n_mb_id)
        .map(|s| s.trim().to_uppercase())
        .unwrap_or_default();
    if name.contains("TIT") {
        4
    } else {
        3
    }
}

fn hex_join(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_udp_tx_preview(conn: &IoConnDto, mb_frames: &[Vec<u8>]) -> Vec<u8> {
    let payload: usize = mb_frames.iter().map(|mb| mb.len()).sum();
    let par = modbus::UdpParams {
        kan: conn.kan,
        speed: conn.speed,
        stop: conn.stop,
        par: conn.par,
        data: conn.data,
        rtu: conn.rtu,
        modem: conn.modem,
        port: conn.port,
        ip: conn.ip.clone(),
        packet_id: 0,
        pkt_type: 0,
        dsr: 0,
        ..Default::default()
    };
    let total_len = 22 + payload;
    let header = modbus::shab(&par, total_len);
    let mut tx = Vec::with_capacity(total_len);
    tx.extend_from_slice(&header);
    for mb in mb_frames {
        tx.extend_from_slice(mb);
    }
    tx
}

fn next_packet_id() -> u8 {
    static PID: AtomicU8 = AtomicU8::new(0);
    PID.fetch_add(1, Ordering::Relaxed)
}

fn next_dsr_id() -> u16 {
    static DSR: AtomicU16 = AtomicU16::new(1);
    let v = DSR.fetch_add(1, Ordering::Relaxed);
    if v == 0 { 1 } else { v }
}

fn send_mb_chunk_over_udp(
    conn: &IoConnDto,
    mb_frames: &[Vec<u8>],
    timeout: Duration,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut payload = 0usize;
    for mb in mb_frames {
        payload += mb.len();
    }
    let packet_id = next_packet_id();
    let dsr = next_dsr_id();
    let par = modbus::UdpParams {
        kan: conn.kan,
        speed: conn.speed,
        stop: conn.stop,
        par: conn.par,
        data: conn.data,
        rtu: conn.rtu,
        modem: conn.modem,
        port: conn.port,
        ip: conn.ip.clone(),
        packet_id,
        pkt_type: 0,
        dsr,
        ..Default::default()
    };
    let total_len = 22 + payload;
    let header = modbus::shab(&par, total_len);
    let mut tx: Vec<u8> = Vec::with_capacity(total_len);
    tx.extend_from_slice(&header);
    for mb in mb_frames {
        tx.extend_from_slice(mb);
    }

    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("udp bind failed: {e}"))?;
    sock.set_read_timeout(Some(timeout))
        .map_err(|e| format!("udp timeout set failed: {e}"))?;
    sock.send_to(&tx, format!("{}:{}", conn.ip, conn.port))
        .map_err(|e| format!("udp send failed: {e}"))?;

    let mut buf = vec![0u8; conn.max_pkt_len.max(65535)];
    loop {
        let (n, _) = sock
            .recv_from(&mut buf)
            .map_err(|e| format!("udp recv failed: {e}"))?;
        if n < 12 {
            continue;
        }
        let pkt = &buf[..n];
        if pkt[3] != packet_id || pkt[4] != 1 {
            // Some gateways do not echo packet_id/dsr exactly.
            // Fallback to first response packet if it is marked as response.
            if pkt[4] == 1 {
                return Ok((tx, pkt.to_vec()));
            }
            continue;
        }
        return Ok((tx, pkt.to_vec()));
    }
}

fn parse_read_words_from_resp(resp: &[u8], expected_func: u8) -> Result<Vec<u16>, String> {
    let mb = modbus::extract_modbus_frame(resp).ok_or_else(|| "short response".to_string())?;
    if mb.len() <= 4 {
        return Err("short response".to_string());
    }
    let ulen = if mb[0] >= 0xF8 { 2 } else { 1 };
    let fi = ulen;
    if mb.len() <= fi + 1 {
        return Err("bad response frame".to_string());
    }
    let func = mb[fi];
    if (func & 0x80) != 0 {
        return Err(format!("modbus exception func={func}"));
    }
    if func != expected_func {
        return Err(format!("unexpected func={}, expected={}", func, expected_func));
    }
    let (byte_count, data_start) = if ulen == 2 {
        if mb.len() < fi + 3 {
            return Err("short 2-byte-len frame".to_string());
        }
        (
            ((mb[fi + 1] as usize) << 8) | (mb[fi + 2] as usize),
            fi + 3,
        )
    } else {
        (mb[fi + 1] as usize, fi + 2)
    };
    if byte_count == 0 || (byte_count % 2) != 0 {
        return Err(format!("bad byte_count={}", byte_count));
    }
    if mb.len() < data_start + byte_count {
        return Err("short data".to_string());
    }
    let mut out = Vec::with_capacity(byte_count / 2);
    let data = &mb[data_start..data_start + byte_count];
    for i in 0..(byte_count / 2) {
        let hi = data[i * 2] as u16;
        let lo = data[i * 2 + 1] as u16;
        out.push((hi << 8) | lo);
    }
    Ok(out)
}

fn reg_words(r: &RegIoDto) -> i32 {
    if matches!(r.tip, 2 | 4 | 5) { 2 } else { 1 }
}

fn decode_reg_value_from_words(words: &[u16], first_addr: i32, r: &RegIoDto) -> Result<f64, String> {
    let idx0 = r.mb - first_addr;
    if idx0 < 0 {
        return Err("address before block".to_string());
    }
    let i0 = idx0 as usize;
    if i0 >= words.len() {
        return Err("address out of block".to_string());
    }
    if matches!(r.tip, 2 | 4 | 5) {
        if i0 + 1 >= words.len() {
            return Err("not enough words for 32-bit value".to_string());
        }
        let hi = words[i0];
        let lo = words[i0 + 1];
        let bytes = [
            ((hi >> 8) & 0xFF) as u8,
            (hi & 0xFF) as u8,
            ((lo >> 8) & 0xFF) as u8,
            (lo & 0xFF) as u8,
        ];
        let v = match r.tip {
            5 => f32::from_be_bytes(bytes) as f64,
            4 => u32::from_be_bytes(bytes) as f64,
            2 => i32::from_be_bytes(bytes) as f64,
            _ => u32::from_be_bytes(bytes) as f64,
        };
        return Ok(v);
    }
    if r.tip == 0 {
        if let Some(bit) = r.bits {
            if (0..=15).contains(&bit) {
                return Ok(((words[i0] >> bit) & 1) as f64);
            }
        }
    }
    let w = words[i0];
    let v = match r.tip {
        1 => (w as i16) as f64,
        _ => w as f64,
    };
    Ok(v)
}

pub async fn api_live_values_real(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<LiveValuesQuery>,
) -> impl IntoResponse {
    let user = match session_user_from_headers(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if !kpz_allowed(&user, q.kpz_id) {
        return api_error(StatusCode::FORBIDDEN, "KPZ_FORBIDDEN", "kpz not allowed");
    }
    let ids = parse_reg_ids(&q.reg_ids);
    if ids.is_empty() {
        return Json(Vec::<LiveValueRealDto>::new()).into_response();
    }
    let Some(conn) = (match load_io_conn_for_kpz(&state.db, q.kpz_id).await {
        Ok(v) => v,
        Err(e) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, "IO_CONFIG_ERROR", e.to_string()),
    }) else {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, "IO_CONFIG_MISSING", "kpz io config not found");
    };
    let regs = match load_regs_io_by_ids(&state.db, &ids).await {
        Ok(v) => v,
        Err(e) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string()),
    };
    let n_mb_by_id = load_n_mb_dict(&state.db).await.unwrap_or_default();
    let by_id: HashMap<i32, RegIoDto> = regs.into_iter().map(|r| (r.id, r)).collect();
    let mut out_map: HashMap<i32, LiveValueRealDto> = HashMap::new();

    #[derive(Clone)]
    struct Group {
        func: u8,
        first_addr: i32,
        regs_count: i32,
        items: Vec<RegIoDto>,
    }

    let mut src: Vec<(u8, RegIoDto)> = Vec::new();
    for id in &ids {
        if let Some(r) = by_id.get(id) {
            let func = read_func_by_n_mb_id(r.n_mb_id, &n_mb_by_id);
            src.push((func, r.clone()));
        }
    }
    src.sort_by_key(|(func, r)| (*func, r.n_mb_id, r.mb));

    let mut groups: Vec<Group> = Vec::new();
    let mut cur_func: Option<u8> = None;
    let mut cur_nmb: Option<i32> = None;
    let mut cur_first = 0i32;
    let mut cur_end = 0i32;
    let mut cur_items: Vec<RegIoDto> = Vec::new();

    let flush_group = |groups: &mut Vec<Group>,
                       cur_func: &mut Option<u8>,
                       cur_nmb: &mut Option<i32>,
                       cur_first: &mut i32,
                       cur_end: &mut i32,
                       cur_items: &mut Vec<RegIoDto>| {
        if let Some(f) = *cur_func {
            let count = (*cur_end - *cur_first).max(1);
            groups.push(Group {
                func: f,
                first_addr: *cur_first,
                regs_count: count,
                items: std::mem::take(cur_items),
            });
        }
        *cur_func = None;
        *cur_nmb = None;
        *cur_first = 0;
        *cur_end = 0;
    };

    for (func, r) in src {
        let w = reg_words(&r).max(1);
        let s = r.mb;
        let e = s + w;
        let compatible = cur_func == Some(func) && cur_nmb == Some(r.n_mb_id) && s <= cur_end;
        if cur_func.is_none() {
            cur_func = Some(func);
            cur_nmb = Some(r.n_mb_id);
            cur_first = s;
            cur_end = e;
            cur_items.push(r);
        } else if compatible {
            if e > cur_end {
                cur_end = e;
            }
            cur_items.push(r);
        } else {
            flush_group(
                &mut groups,
                &mut cur_func,
                &mut cur_nmb,
                &mut cur_first,
                &mut cur_end,
                &mut cur_items,
            );
            cur_func = Some(func);
            cur_nmb = Some(r.n_mb_id);
            cur_first = s;
            cur_end = e;
            cur_items.push(r);
        }
    }
    flush_group(
        &mut groups,
        &mut cur_func,
        &mut cur_nmb,
        &mut cur_first,
        &mut cur_end,
        &mut cur_items,
    );

    let mut plans: Vec<(Group, Vec<u8>)> = Vec::new();
    for g in groups {
        let mb = match modbus::sout_mb_only(conn.rtu, g.func, g.first_addr, g.regs_count as u16, None) {
            Ok(v) => v,
            Err(e) => {
                for r in &g.items {
                    out_map.insert(
                        r.id,
                        LiveValueRealDto {
                            reg_id: r.id,
                            val_num: None,
                            io_ip: Some(conn.ip.clone()),
                            io_modem: Some(conn.modem),
                            req_hex: None,
                            resp_hex: None,
                            error: Some(e.clone()),
                        },
                    );
                }
                continue;
            }
        };
        plans.push((g, mb));
    }

    let mb_cmds: Vec<Vec<u8>> = plans.iter().map(|(_, mb)| mb.clone()).collect();
    let mb_chunks = match modbus::build_mb_chunks(&mb_cmds, conn.max_pkt_len.max(64)) {
        Ok(v) => v,
        Err(e) => {
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, "CHUNK_BUILD_FAILED", format!("chunk build failed: {}", e));
        }
    };

    let mut plan_offset = 0usize;
    for chunk_frames in mb_chunks {
        let chunk_len = chunk_frames.len();
        let chunk_plans = &plans[plan_offset..(plan_offset + chunk_len)];
        plan_offset += chunk_len;

        match send_mb_chunk_over_udp(&conn, &chunk_frames, Duration::from_millis(5000)) {
            Ok((tx, resp)) => {
                let req_hex = hex_join(&tx);
                let resp_hex = hex_join(&resp);
                let parts = modbus::split_rx_to_virtual(&resp);
                for i in 0..chunk_plans.len() {
                    let (g, _) = &chunk_plans[i];
                    let part = parts.get(i);
                    match part {
                        Some(pkt) => match parse_read_words_from_resp(pkt, g.func) {
                            Ok(words) => {
                                for r in &g.items {
                                    match decode_reg_value_from_words(&words, g.first_addr, r) {
                                        Ok(v) => {
                                            out_map.insert(
                                                r.id,
                                                LiveValueRealDto {
                                                    reg_id: r.id,
                                                    val_num: Some(v),
                                                    io_ip: Some(conn.ip.clone()),
                                                    io_modem: Some(conn.modem),
                                                    req_hex: Some(req_hex.clone()),
                                                    resp_hex: Some(resp_hex.clone()),
                                                    error: None,
                                                },
                                            );
                                        }
                                        Err(e) => {
                                            out_map.insert(
                                                r.id,
                                                LiveValueRealDto {
                                                    reg_id: r.id,
                                                    val_num: None,
                                                    io_ip: Some(conn.ip.clone()),
                                                    io_modem: Some(conn.modem),
                                                    req_hex: Some(req_hex.clone()),
                                                    resp_hex: Some(resp_hex.clone()),
                                                    error: Some(e),
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                for r in &g.items {
                                    out_map.insert(
                                        r.id,
                                        LiveValueRealDto {
                                            reg_id: r.id,
                                            val_num: None,
                                            io_ip: Some(conn.ip.clone()),
                                            io_modem: Some(conn.modem),
                                            req_hex: Some(req_hex.clone()),
                                            resp_hex: Some(resp_hex.clone()),
                                            error: Some(e.clone()),
                                        },
                                    );
                                }
                            }
                        },
                        None => {
                            for r in &g.items {
                                out_map.insert(
                                    r.id,
                                    LiveValueRealDto {
                                        reg_id: r.id,
                                        val_num: None,
                                        io_ip: Some(conn.ip.clone()),
                                        io_modem: Some(conn.modem),
                                        req_hex: Some(req_hex.clone()),
                                        resp_hex: Some(resp_hex.clone()),
                                        error: Some("responses < commands".to_string()),
                                    },
                                );
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let req_hex = hex_join(&build_udp_tx_preview(&conn, &chunk_frames));
                for (g, _) in chunk_plans {
                    for r in &g.items {
                        out_map.insert(
                            r.id,
                            LiveValueRealDto {
                                reg_id: r.id,
                                val_num: None,
                                io_ip: Some(conn.ip.clone()),
                                io_modem: Some(conn.modem),
                                req_hex: Some(req_hex.clone()),
                                resp_hex: None,
                                error: Some(e.clone()),
                            },
                        );
                    }
                }
            }
        }
    }

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(v) = out_map.remove(&id) {
            out.push(v);
        } else {
            out.push(LiveValueRealDto {
                reg_id: id,
                val_num: None,
                io_ip: Some(conn.ip.clone()),
                io_modem: Some(conn.modem),
                req_hex: None,
                resp_hex: None,
                error: Some("reg not found".to_string()),
            });
        }
    }
    Json(out).into_response()
}

pub async fn api_alarm_rules(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<UiWindowsQuery>,
) -> impl IntoResponse {
    let user = match session_user_from_headers(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if !kpz_allowed(&user, q.kpz_id) {
        return api_error(StatusCode::FORBIDDEN, "KPZ_FORBIDDEN", "kpz not allowed");
    }
    match load_alarm_rules_for_kpz(&state.db, q.kpz_id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string()),
    }
}

pub async fn api_actions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    match session_user_from_headers(&state, &headers).await {
        Ok(_) => {}
        Err(resp) => return resp,
    }
    match load_recent_web_actions(&state.db, 50).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, "DB_QUERY_FAILED", e.to_string()),
    }
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn resolve_ui_image_path(rel_path: &str) -> Option<PathBuf> {
    let requested = rel_path.replace('\\', "/");
    let path = Path::new(&requested);
    if path.is_absolute() {
        return None;
    }
    if path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return None;
    }

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("ui_images").join(path));
    }
    if let Ok(exe) = std::env::current_exe() {
        for base in exe.ancestors().filter(|x| x.is_dir()) {
            candidates.push(base.join("ui_images").join(path));
        }
    }
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_images").join(path));

    candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .or_else(|| candidates.into_iter().next())
}

#[derive(serde::Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<WsQuery>,
) -> impl IntoResponse {
    let user = match session_user_from_headers(&state, &headers).await {
        Ok(u) => u,
        Err(_first_err) => {
            if let Some(token) = q.token.as_ref().filter(|t| !t.is_empty()) {
                match load_web_session_user(&state.db, token).await {
                    Ok(Some(u)) => u,
                    _ => return api_error(StatusCode::UNAUTHORIZED, "AUTH_REQUIRED", "login required"),
                }
            } else {
                return api_error(StatusCode::UNAUTHORIZED, "AUTH_REQUIRED", "login required");
            }
        }
    };
    ws.on_upgrade(move |socket| handle_ws(socket, state, user))
}

async fn handle_ws(mut socket: WebSocket, state: AppState, user: crate::models::WebSessionUserDto) {
    loop {
        let msg = match socket.recv().await {
            Some(Ok(Message::Text(t))) => t,
            Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
            _ => continue,
        };

        #[derive(serde::Deserialize)]
        struct WsPollReq {
            r#type: String,
            kpz_id: i32,
            reg_ids: String,
        }

        let req: WsPollReq = match serde_json::from_str(&msg) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if !kpz_allowed(&user, req.kpz_id) {
            let _ = socket.send(Message::Text(r#"{"type":"error","code":"KPZ_FORBIDDEN"}"#.into())).await;
            continue;
        }

        let ids = parse_reg_ids(&req.reg_ids);
        if ids.is_empty() {
            let _ = socket.send(Message::Text(r#"{"type":"error","code":"NO_IDS"}"#.into())).await;
            continue;
        }

        match req.r#type.as_str() {
            "poll_db" => {
                match load_live_values(&state.db, req.kpz_id, &ids).await {
                    Ok(rows) => {
                        if let Ok(text) = serde_json::to_string(&serde_json::json!({"type":"poll_result","source":"db","rows":rows})) {
                            let _ = socket.send(Message::Text(text.into())).await;
                        }
                    }
                    Err(e) => {
                        let _ = socket.send(Message::Text(format!(r#"{{"type":"error","code":"DB_QUERY_FAILED","message":"{}"}}"#, e).into())).await;
                    }
                }
            }
            "poll_real" => {
                match poll_real_internal(&state, &user, req.kpz_id, &ids).await {
                    Ok(rows) => {
                        if let Ok(text) = serde_json::to_string(&serde_json::json!({"type":"poll_result","source":"real","rows":rows})) {
                            let _ = socket.send(Message::Text(text.into())).await;
                        }
                    }
                    Err(e) => {
                        let _ = socket.send(Message::Text(format!(r#"{{"type":"error","code":"REAL_POLL_FAILED","message":"{}"}}"#, e).into())).await;
                    }
                }
            }
            _ => {}
        }
    }
}

async fn poll_real_internal(
    state: &AppState,
    _user: &crate::models::WebSessionUserDto,
    kpz_id: i32,
    ids: &[i32],
) -> Result<Vec<LiveValueRealDto>, String> {
    let Some(conn) = load_io_conn_for_kpz(&state.db, kpz_id).await.map_err(|e| e.to_string())? else {
        return Err("IO config not found".to_string());
    };
    let regs = load_regs_io_by_ids(&state.db, ids).await.map_err(|e| e.to_string())?;
    let n_mb_by_id = load_n_mb_dict(&state.db).await.unwrap_or_default();
    let by_id: HashMap<i32, RegIoDto> = regs.into_iter().map(|r| (r.id, r)).collect();

    let mut src: Vec<(u8, RegIoDto)> = Vec::new();
    for id in ids {
        if let Some(r) = by_id.get(id) {
            let func = read_func_by_n_mb_id(r.n_mb_id, &n_mb_by_id);
            src.push((func, r.clone()));
        }
    }
    src.sort_by_key(|(func, r)| (*func, r.n_mb_id, r.mb));

    let mut out_map: HashMap<i32, LiveValueRealDto> = HashMap::new();
    let mut groups: Vec<GroupPlan> = Vec::new();
    let mut cur_func: Option<u8> = None;
    let mut cur_nmb: Option<i32> = None;
    let mut cur_first = 0i32;
    let mut cur_end = 0i32;
    let mut cur_items: Vec<RegIoDto> = Vec::new();

    let flush_group = |groups: &mut Vec<GroupPlan>,
                        cur_func: &mut Option<u8>,
                        cur_nmb: &mut Option<i32>,
                        cur_first: &mut i32,
                        cur_end: &mut i32,
                        cur_items: &mut Vec<RegIoDto>| {
        if let Some(f) = *cur_func {
            groups.push(GroupPlan { func: f, first_addr: *cur_first, regs_count: (*cur_end - *cur_first).max(1), items: std::mem::take(cur_items) });
        }
        *cur_func = None;
        *cur_nmb = None;
        *cur_first = 0;
        *cur_end = 0;
    };

    for (func, r) in src {
        let w = reg_words(&r).max(1);
        let s = r.mb;
        let e = s + w;
        let compatible = cur_func == Some(func) && cur_nmb == Some(r.n_mb_id) && s <= cur_end;
        if cur_func.is_none() {
            cur_func = Some(func);
            cur_nmb = Some(r.n_mb_id);
            cur_first = s;
            cur_end = e;
            cur_items.push(r);
        } else if compatible {
            if e > cur_end { cur_end = e; }
            cur_items.push(r);
        } else {
            flush_group(&mut groups, &mut cur_func, &mut cur_nmb, &mut cur_first, &mut cur_end, &mut cur_items);
            cur_func = Some(func);
            cur_nmb = Some(r.n_mb_id);
            cur_first = s;
            cur_end = e;
            cur_items.push(r);
        }
    }
    flush_group(&mut groups, &mut cur_func, &mut cur_nmb, &mut cur_first, &mut cur_end, &mut cur_items);

    let mut plans: Vec<(GroupPlan, Vec<u8>)> = Vec::new();
    for g in groups {
        match modbus::sout_mb_only(conn.rtu, g.func, g.first_addr, g.regs_count as u16, None) {
            Ok(mb) => plans.push((g, mb)),
            Err(e) => {
                for r in &g.items {
                    out_map.insert(r.id, LiveValueRealDto {
                        reg_id: r.id, val_num: None,
                        io_ip: Some(conn.ip.clone()), io_modem: Some(conn.modem),
                        req_hex: None, resp_hex: None, error: Some(e.clone()),
                    });
                }
            }
        }
    }

    let mb_cmds: Vec<Vec<u8>> = plans.iter().map(|(_, mb)| mb.clone()).collect();
    let mb_chunks = match modbus::build_mb_chunks(&mb_cmds, conn.max_pkt_len.max(64)) {
        Ok(v) => v,
        Err(e) => return Err(e),
    };

    let mut plan_offset = 0usize;
    for chunk_frames in mb_chunks {
        let chunk_len = chunk_frames.len();
        let chunk_plans = &plans[plan_offset..(plan_offset + chunk_len)];
        plan_offset += chunk_len;
        match send_mb_chunk_over_udp(&conn, &chunk_frames, Duration::from_millis(5000)) {
            Ok((tx, resp)) => {
                let req_hex = hex_join(&tx);
                let resp_hex = hex_join(&resp);
                let parts = modbus::split_rx_to_virtual(&resp);
                for i in 0..chunk_plans.len() {
                    let (g, _) = &chunk_plans[i];
                    match parts.get(i) {
                        Some(pkt) => match parse_read_words_from_resp(pkt, g.func) {
                            Ok(words) => {
                                for r in &g.items {
                                    let v = decode_reg_value_from_words(&words, g.first_addr, r).ok();
                                    out_map.insert(r.id, LiveValueRealDto {
                                        reg_id: r.id, val_num: v,
                                        io_ip: Some(conn.ip.clone()), io_modem: Some(conn.modem),
                                        req_hex: Some(req_hex.clone()), resp_hex: Some(resp_hex.clone()), error: None,
                                    });
                                }
                            }
                            Err(e) => {
                                for r in &g.items {
                                    out_map.insert(r.id, LiveValueRealDto {
                                        reg_id: r.id, val_num: None,
                                        io_ip: Some(conn.ip.clone()), io_modem: Some(conn.modem),
                                        req_hex: Some(req_hex.clone()), resp_hex: Some(resp_hex.clone()), error: Some(e.clone()),
                                    });
                                }
                            }
                        },
                        None => {
                            for r in &g.items {
                                out_map.insert(r.id, LiveValueRealDto {
                                    reg_id: r.id, val_num: None,
                                    io_ip: Some(conn.ip.clone()), io_modem: Some(conn.modem),
                                    req_hex: Some(req_hex.clone()), resp_hex: Some(resp_hex.clone()), error: Some("responses < commands".to_string()),
                                });
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let tx = build_udp_tx_preview(&conn, &chunk_frames);
                let req_hex = hex_join(&tx);
                for (g, _) in chunk_plans {
                    for r in &g.items {
                        out_map.insert(r.id, LiveValueRealDto {
                            reg_id: r.id, val_num: None,
                            io_ip: Some(conn.ip.clone()), io_modem: Some(conn.modem),
                            req_hex: Some(req_hex.clone()), resp_hex: None, error: Some(e.clone()),
                        });
                    }
                }
            }
        }
    }

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(v) = out_map.remove(id) {
            out.push(v);
        } else {
            out.push(LiveValueRealDto {
                reg_id: *id, val_num: None,
                io_ip: Some(conn.ip.clone()), io_modem: Some(conn.modem),
                req_hex: None, resp_hex: None, error: Some("reg not found".to_string()),
            });
        }
    }
    Ok(out)
}

struct GroupPlan {
    func: u8,
    first_addr: i32,
    regs_count: i32,
    items: Vec<RegIoDto>,
}

pub async fn ui_image(AxumPath(path): AxumPath<String>) -> impl IntoResponse {
    let Some(full_path) = resolve_ui_image_path(&path) else {
        return api_error(StatusCode::BAD_REQUEST, "BAD_IMAGE_PATH", "invalid image path");
    };
    let content_type = content_type_for_path(&full_path);
    match std::fs::read(&full_path) {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "no-cache, must-revalidate")],
            Body::from(bytes),
        )
            .into_response(),
        Err(_) => api_error(StatusCode::NOT_FOUND, "IMAGE_NOT_FOUND", "image not found"),
    }
}
