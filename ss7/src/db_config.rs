use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use serde::Deserialize;

pub(crate) const PG_HOST: &str = "localhost";
pub(crate) const PG_PORT: u16 = 5432;
pub(crate) const PG_DB: &str = "postgres_restored";

#[derive(Debug, Deserialize)]
pub(crate) struct FileConfig {
    pub(crate) pg_host: Option<String>,
    pub(crate) pg_port: Option<u16>,
    pub(crate) pg_db: Option<String>,
    pub(crate) pg_user: String,
    pub(crate) pg_pass: String,
}

pub(crate) fn load_file_config() -> Result<FileConfig> {
    for path in candidate_paths() {
        if !path.exists() {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| anyhow!("failed to read {}: {}", path.display(), e))?;
        let cfg: FileConfig = toml::from_str(&raw)
            .map_err(|e| anyhow!("invalid TOML in {}: {}", path.display(), e))?;
        return Ok(cfg);
    }
    Err(anyhow!(
        "DB config not found. Set PG_* env vars or create ss7.toml рядом с exe/project."
    ))
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut out = vec![PathBuf::from("ss7.toml")];
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        out.push(dir.join("ss7.toml"));
    }
    out
}
