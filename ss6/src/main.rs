mod config;
mod db;
mod handlers;
mod models;
mod modbus;
mod web;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::http::{HeaderValue, header};
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use tokio_postgres::NoTls;

use crate::db::{count_web_users, ensure_web_auth_schema, insert_web_user};
use crate::handlers::{
    api_alarm_rules, api_arx_series, api_groups, api_kpz, api_live_values, api_live_values_real,
    api_login, api_logout, api_regs, api_tu_write, api_ui_bindings, api_ui_windows, api_write_value,
    api_actions, hash_password_argon2, index, login_page, make_password_salt, require_auth, static_api_js,
    static_app_css, static_app_js, static_chart_controller_js, static_meta_state_js,
    static_preview_modals_js, static_preview_poll_js, static_preview_scale_js,
    static_preview_scene_js, ui_image, ws_handler, AppState,
};

async fn csp_headers(req: axum::http::Request<axum::body::Body>, next: axum::middleware::Next) -> axum::response::Response {
    let mut resp = next.run(req).await;
    let csp = "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval' https://cdn.jsdelivr.net; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";
    if let Ok(val) = HeaderValue::from_str(csp) {
        resp.headers_mut().insert(header::CONTENT_SECURITY_POLICY, val);
    }
    resp
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = config::Config::from_env()?;
    let conn_str = cfg.pg_conn_string();

    let (client, conn) = tokio_postgres::connect(&conn_str, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("db connection error: {e}");
        }
    });

    ensure_web_auth_schema(&client).await?;
    if count_web_users(&client).await? == 0 {
        let salt = make_password_salt();
        let hash = hash_password_argon2(&salt, &cfg.web_admin_password)?;
        insert_web_user(&client, &cfg.web_admin_login, &salt, &hash, "admin").await?;
        println!(
            "ss6 web bootstrap user created: login='{}' password='{}'",
            cfg.web_admin_login, cfg.web_admin_password
        );
    }

    let state = AppState {
        db: Arc::new(client),
    };

    let protected = Router::new()
        .route("/", get(index))
        .route("/api/kpz", get(api_kpz))
        .route("/api/groups", get(api_groups))
        .route("/api/regs", get(api_regs))
        .route("/api/arx_series", get(api_arx_series))
        .route("/api/ui_windows", get(api_ui_windows))
        .route("/api/ui_bindings", get(api_ui_bindings))
        .route("/api/live_values", get(api_live_values))
        .route("/api/live_values_real", get(api_live_values_real))
        .route("/api/tu_write", post(api_tu_write))
        .route("/api/write_value", post(api_write_value))
        .route("/api/alarm_rules", get(api_alarm_rules))
        .route("/api/actions", get(api_actions))
        .route("/ui_images/{*path}", get(ui_image))
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    let app = Router::new()
        .route("/login", get(login_page).post(api_login))
        .route("/logout", post(api_logout))
        .route("/static/app.js", get(static_app_js))
        .route("/static/app.css", get(static_app_css))
        .route("/static/api.js", get(static_api_js))
        .route("/static/chart_controller.js", get(static_chart_controller_js))
        .route("/static/meta_state.js", get(static_meta_state_js))
        .route("/static/preview_modals.js", get(static_preview_modals_js))
        .route("/static/preview_scale.js", get(static_preview_scale_js))
        .route("/static/preview_poll.js", get(static_preview_poll_js))
        .route("/static/preview_scene.js", get(static_preview_scene_js))
        .route("/ws", get(ws_handler))
        .merge(protected)
        .layer(middleware::from_fn(csp_headers))
        .with_state(state);

    let addr: SocketAddr = "127.0.0.1:8097".parse()?;
    println!("ss6 running at http://{}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
