use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FileConfig {
    pg_host: String,
    pg_port: u16,
    pg_db: String,
    pg_user: String,
    pg_pass: String,
}

pub struct Config {
    pub pg_host: String,
    pub pg_port: u16,
    pub pg_db: String,
    pub pg_user: String,
    pub pg_pass: String,
    pub web_admin_login: String,
    pub web_admin_password: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let web_admin_login = env::var("WEB_ADMIN_LOGIN").unwrap_or_else(|_| "admin".to_string());
        let web_admin_password = env::var("WEB_ADMIN_PASSWORD").unwrap_or_else(|_| "admin".to_string());
        // 1) Prefer explicit env vars.
        let pg_host = env::var("PG_HOST").ok();
        let pg_port = env::var("PG_PORT").ok();
        let pg_db = env::var("PG_DB").ok();
        let pg_user = env::var("PG_USER").ok();
        let pg_pass = env::var("PG_PASS").ok();
        if let (Some(pg_host), Some(pg_port), Some(pg_db), Some(pg_user), Some(pg_pass)) =
            (pg_host, pg_port, pg_db, pg_user, pg_pass)
        {
            let pg_port = pg_port
                .parse::<u16>()
                .map_err(|_| anyhow!("PG_PORT must be valid u16"))?;
            return Ok(Self {
                pg_host,
                pg_port,
                pg_db,
                pg_user,
                pg_pass,
                web_admin_login,
                web_admin_password,
            });
        }

        // 2) Fallback to TOML config file.
        // Search order:
        //   - ./ss6.toml (current working directory)
        //   - <exe_dir>/ss6.toml
        for path in candidate_paths() {
            if !path.exists() {
                continue;
            }
            let raw = fs::read_to_string(&path)
                .map_err(|e| anyhow!("failed to read {}: {}", path.display(), e))?;
            let file_cfg: FileConfig = toml::from_str(&raw)
                .map_err(|e| anyhow!("invalid TOML in {}: {}", path.display(), e))?;
            return Ok(Self {
                pg_host: file_cfg.pg_host,
                pg_port: file_cfg.pg_port,
                pg_db: file_cfg.pg_db,
                pg_user: file_cfg.pg_user,
                pg_pass: file_cfg.pg_pass,
                web_admin_login: web_admin_login.clone(),
                web_admin_password: web_admin_password.clone(),
            });
        }

        Err(anyhow!(
            "DB config not found. Set PG_* env vars or create ss6.toml рядом с exe/project."
        ))
    }

    pub fn pg_conn_string(&self) -> String {
        format!(
            "host={} port={} user={} password={} dbname={}",
            self.pg_host, self.pg_port, self.pg_user, self.pg_pass, self.pg_db
        )
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    out.push(PathBuf::from("ss6.toml"));
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join("ss6.toml"));
        }
    }
    out
}
