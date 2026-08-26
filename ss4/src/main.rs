#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use anyhow::Result;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

mod db;
mod db_queries;
mod modbus;
mod modbus_service;
mod mqtt_publisher;
mod poller;
mod reg;
mod scheduler;
mod script;
mod script_cache;
mod telegram_notifier;
mod types;
mod udp_transport;

fn env_u64(get_env: &dyn Fn(&str) -> Option<String>, name: &str, default: u64) -> u64 {
    get_env(name)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_usize(get_env: &dyn Fn(&str) -> Option<String>, name: &str, default: usize) -> usize {
    get_env(name)
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

fn env_bool(get_env: &dyn Fn(&str) -> Option<String>, name: &str, default: bool) -> bool {
    get_env(name)
        .and_then(|v| match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

fn env_required(name: &str) -> Result<String> {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => Ok(v),
        _ => Err(anyhow::anyhow!("required env var {name} is missing")),
    }
}

#[derive(Debug, Deserialize)]
struct FileDbConfig {
    host: String,
    port: u16,
    db: String,
    user: String,
    pass: String,
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    db: FileDbConfig,
    log: Option<FileLogConfig>,
    telegram: Option<FileTelegramConfig>,
    mqtt: Option<FileMqttConfig>,
    scheduler: Option<FileSchedulerConfig>,
}

#[derive(Debug, Deserialize)]
struct FileLogConfig {
    level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FileTelegramConfig {
    enabled: Option<bool>,
    bot_token: Option<String>,
    bot_token_env: Option<String>,
    queue_cap: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct FileMqttConfig {
    enabled: Option<bool>,
    host: Option<String>,
    port: Option<u16>,
    client_id: Option<String>,
    username: Option<String>,
    username_env: Option<String>,
    password: Option<String>,
    password_env: Option<String>,
    topic_prefix: Option<String>,
    queue_cap: Option<usize>,
    qos: Option<u8>,
    retain_health: Option<bool>,
    publish_values: Option<bool>,
    publish_alarms: Option<bool>,
    value_kpz_ids: Option<Vec<i32>>,
    value_group_ids: Option<Vec<i32>>,
    value_reg_ids: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize)]
struct FileSchedulerConfig {
    pool_size: Option<usize>,
    tick_ms: Option<u64>,
    sync_period_sec: Option<u64>,
    max_queue: Option<usize>,
    max_inflight: Option<usize>,
    auto_inflight: Option<bool>,
    auto_inflight_max: Option<usize>,
    auto_inflight_backlog_per_slot: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct SchedulerCfg {
    pool_size: usize,
    tick_ms: u64,
    sync_period_sec: u64,
    max_queue: usize,
    max_inflight: usize,
}

fn config_path_near_exe() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("failed to resolve executable directory"))?;
    Ok(dir.join("ss4.toml"))
}

fn config_candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(path) = config_path_near_exe() {
        paths.push(path);
    }
    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join("ss4.toml"));
    }
    paths.dedup();
    paths
}

fn load_file_config() -> Result<Option<FileConfig>> {
    for path in config_candidate_paths() {
        if path.exists() {
            let raw = fs::read_to_string(&path)?;
            let cfg: FileConfig = toml::from_str(&raw)?;
            return Ok(Some(cfg));
        }
    }
    Ok(None)
}

fn load_db_config() -> Result<db::DbConfig> {
    if let Some(cfg) = load_file_config()? {
        return Ok(db::DbConfig {
            host: cfg.db.host,
            port: cfg.db.port,
            db: cfg.db.db,
            user: cfg.db.user,
            pass: cfg.db.pass,
        });
    }

    Ok(db::DbConfig {
        host: env_required("PG_HOST")?,
        port: env_required("PG_PORT")?.parse()?,
        db: env_required("PG_DB")?,
        user: env_required("PG_USER")?,
        pass: env_required("PG_PASS")?,
    })
}

fn load_log_filter_from_file() -> Option<EnvFilter> {
    let cfg = load_file_config().ok().flatten()?;
    let level = cfg.log?.level?.trim().to_string();
    if level.is_empty() {
        return None;
    }
    EnvFilter::try_new(level).ok()
}

fn load_telegram_notifier_from_file() -> Option<telegram_notifier::TelegramNotifier> {
    let cfg = load_file_config().ok().flatten()?;
    let tg = cfg.telegram?;
    if !tg.enabled.unwrap_or(false) {
        return None;
    }

    let bot_token = resolve_telegram_bot_token(&tg, |name| std::env::var(name).ok())?;
    let queue_cap = tg.queue_cap.unwrap_or(200);
    Some(telegram_notifier::TelegramNotifier::new(
        bot_token, None, queue_cap,
    ))
}

fn load_mqtt_publisher_from_file() -> Option<mqtt_publisher::MqttPublisher> {
    let cfg = load_file_config().ok().flatten()?;
    let mqtt = cfg.mqtt?;
    if !mqtt.enabled.unwrap_or(false) {
        return None;
    }

    let publisher_cfg = resolve_mqtt_config(&mqtt, &|name| std::env::var(name).ok());
    Some(mqtt_publisher::MqttPublisher::start(publisher_cfg))
}

fn resolve_mqtt_config(
    mqtt: &FileMqttConfig,
    get_env: &dyn Fn(&str) -> Option<String>,
) -> mqtt_publisher::MqttPublisherConfig {
    let username = resolve_optional_secret(
        mqtt.username_env.as_deref(),
        mqtt.username.as_deref(),
        "MQTT_USER",
        get_env,
    );
    let password = resolve_optional_secret(
        mqtt.password_env.as_deref(),
        mqtt.password.as_deref(),
        "MQTT_PASS",
        get_env,
    );
    let qos = match mqtt.qos.unwrap_or(1) {
        0 => mqtt_publisher::MqttQos::Qos0,
        2 => mqtt_publisher::MqttQos::Qos2,
        _ => mqtt_publisher::MqttQos::Qos1,
    };

    mqtt_publisher::MqttPublisherConfig {
        host: mqtt
            .host
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("127.0.0.1")
            .to_string(),
        port: mqtt.port.unwrap_or(1883),
        client_id: mqtt
            .client_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("ss4")
            .to_string(),
        username,
        password,
        topic_prefix: mqtt_publisher::normalize_topic_prefix(
            mqtt.topic_prefix.as_deref().unwrap_or("ss4/v1"),
        ),
        queue_cap: mqtt.queue_cap.unwrap_or(1000).max(1),
        qos,
        retain_health: mqtt.retain_health.unwrap_or(true),
        publish_values: mqtt.publish_values.unwrap_or(true),
        publish_alarms: mqtt.publish_alarms.unwrap_or(true),
        value_kpz_ids: positive_id_set(mqtt.value_kpz_ids.as_deref()),
        value_group_ids: positive_id_set(mqtt.value_group_ids.as_deref()),
        value_reg_ids: positive_id_set(mqtt.value_reg_ids.as_deref()),
    }
}

fn positive_id_set(values: Option<&[i32]>) -> HashSet<i32> {
    values
        .unwrap_or(&[])
        .iter()
        .copied()
        .filter(|v| *v > 0)
        .collect()
}

fn resolve_optional_secret(
    env_name: Option<&str>,
    file_value: Option<&str>,
    default_env_name: &str,
    get_env: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let env_name = env_name
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(default_env_name);
    get_env(env_name)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| {
            file_value
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        })
}

fn resolve_telegram_bot_token(
    tg: &FileTelegramConfig,
    get_env: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let env_name = tg
        .bot_token_env
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("TELEGRAM_BOT_TOKEN");
    let bot_token = get_env(env_name)
        .filter(|v| !v.trim().is_empty())
        .or_else(|| tg.bot_token.as_deref().map(|v| v.trim().to_string()))?;
    if bot_token.is_empty() {
        return None;
    }
    Some(bot_token)
}

fn load_scheduler_config() -> Result<SchedulerCfg> {
    let file_config = load_file_config()?;
    let file_scheduler = file_config.as_ref().and_then(|cfg| cfg.scheduler.as_ref());
    let (cfg, has_auto_inflight_config) =
        resolve_scheduler_config(file_scheduler, &|name| std::env::var(name).ok());

    if has_auto_inflight_config {
        tracing::warn!(
            "auto inflight config is present but not implemented in runtime; using fixed max_inflight"
        );
    }

    Ok(cfg)
}

fn resolve_scheduler_config(
    file_scheduler: Option<&FileSchedulerConfig>,
    get_env: &dyn Fn(&str) -> Option<String>,
) -> (SchedulerCfg, bool) {
    let pool_size = file_scheduler
        .as_ref()
        .and_then(|s| s.pool_size)
        .unwrap_or_else(|| env_usize(get_env, "SCHED_POOL_SIZE", 420))
        .max(1);
    let tick_ms = file_scheduler
        .as_ref()
        .and_then(|s| s.tick_ms)
        .unwrap_or_else(|| env_u64(get_env, "SCHED_TICK_MS", 250))
        .max(1);
    let sync_period_sec = file_scheduler
        .as_ref()
        .and_then(|s| s.sync_period_sec)
        .unwrap_or_else(|| env_u64(get_env, "SCHED_SYNC_PERIOD_SEC", 2))
        .max(1);
    let max_queue = file_scheduler
        .as_ref()
        .and_then(|s| s.max_queue)
        .unwrap_or_else(|| env_usize(get_env, "SCHED_MAX_QUEUE", 10000))
        .max(1);
    let max_inflight = file_scheduler
        .as_ref()
        .and_then(|s| s.max_inflight)
        .unwrap_or_else(|| env_usize(get_env, "SCHED_MAX_INFLIGHT", pool_size))
        .max(1);

    let auto_inflight = file_scheduler
        .as_ref()
        .and_then(|s| s.auto_inflight)
        .unwrap_or_else(|| env_bool(get_env, "SCHED_AUTO_INFLIGHT", false));
    let auto_inflight_max = file_scheduler
        .as_ref()
        .and_then(|s| s.auto_inflight_max)
        .or_else(|| get_env("SCHED_AUTO_INFLIGHT_MAX").and_then(|v| v.parse::<usize>().ok()));
    let auto_inflight_backlog_per_slot = file_scheduler
        .as_ref()
        .and_then(|s| s.auto_inflight_backlog_per_slot)
        .or_else(|| {
            get_env("SCHED_AUTO_INFLIGHT_BACKLOG_PER_SLOT").and_then(|v| v.parse::<usize>().ok())
        });

    (
        SchedulerCfg {
            pool_size,
            tick_ms,
            sync_period_sec,
            max_queue,
            max_inflight,
        },
        auto_inflight || auto_inflight_max.is_some() || auto_inflight_backlog_per_slot.is_some(),
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| {
            load_log_filter_from_file().ok_or_else(|| anyhow::anyhow!("no log level in file"))
        })
        .unwrap_or_else(|_| EnvFilter::new("info,ss4=debug"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .init();

    tracing::info!("ss4 starting (rust poller/scheduler)");
    let cfg = load_db_config()?;
    let telegram = load_telegram_notifier_from_file();
    if telegram.is_some() {
        tracing::info!("telegram notifier enabled");
    }
    let mqtt = load_mqtt_publisher_from_file();
    if mqtt.is_some() {
        tracing::info!("mqtt publisher enabled");
    }

    let client = db::connect(&cfg).await?;
    let sched_cfg = load_scheduler_config()?;
    tracing::info!(
        pool_size = sched_cfg.pool_size,
        tick_ms = sched_cfg.tick_ms,
        sync_period_sec = sched_cfg.sync_period_sec,
        max_queue = sched_cfg.max_queue,
        max_inflight = sched_cfg.max_inflight,
        no_response_cfg = "db",
        "scheduler config"
    );
    let sched = scheduler::Scheduler {
        pool_size: sched_cfg.pool_size,
        tick_ms: sched_cfg.tick_ms,
        sync_period_sec: sched_cfg.sync_period_sec,
        max_queue: sched_cfg.max_queue,
        max_inflight: sched_cfg.max_inflight,
        no_response_failures: 3,
        no_response_backoff_sec: 600,
        telegram,
        mqtt,
    };
    sched.run(client).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn telegram_cfg(bot_token: Option<&str>, bot_token_env: Option<&str>) -> FileTelegramConfig {
        FileTelegramConfig {
            enabled: Some(true),
            bot_token: bot_token.map(str::to_string),
            bot_token_env: bot_token_env.map(str::to_string),
            queue_cap: None,
        }
    }

    fn mqtt_cfg() -> FileMqttConfig {
        FileMqttConfig {
            enabled: Some(true),
            host: None,
            port: None,
            client_id: None,
            username: None,
            username_env: None,
            password: None,
            password_env: None,
            topic_prefix: None,
            queue_cap: None,
            qos: None,
            retain_health: None,
            publish_values: None,
            publish_alarms: None,
            value_kpz_ids: None,
            value_group_ids: None,
            value_reg_ids: None,
        }
    }

    #[test]
    fn telegram_token_prefers_configured_env_var() {
        let cfg = telegram_cfg(Some("file-token"), Some("SS4_TG_TOKEN"));
        let token = resolve_telegram_bot_token(&cfg, |name| {
            (name == "SS4_TG_TOKEN").then(|| "env-token".to_string())
        });

        assert_eq!(token.as_deref(), Some("env-token"));
    }

    #[test]
    fn telegram_token_uses_default_env_var_when_name_is_missing() {
        let cfg = telegram_cfg(Some("file-token"), None);
        let token = resolve_telegram_bot_token(&cfg, |name| {
            (name == "TELEGRAM_BOT_TOKEN").then(|| "default-env-token".to_string())
        });

        assert_eq!(token.as_deref(), Some("default-env-token"));
    }

    #[test]
    fn telegram_token_falls_back_to_file_value() {
        let cfg = telegram_cfg(Some("  file-token  "), Some("SS4_TG_TOKEN"));
        let token = resolve_telegram_bot_token(&cfg, |_| None);

        assert_eq!(token.as_deref(), Some("file-token"));
    }

    #[test]
    fn mqtt_config_defaults_are_safe_for_local_broker() {
        let cfg = mqtt_cfg();
        let resolved = resolve_mqtt_config(&cfg, &|_| None);

        assert_eq!(resolved.host, "127.0.0.1");
        assert_eq!(resolved.port, 1883);
        assert_eq!(resolved.client_id, "ss4");
        assert_eq!(resolved.topic_prefix, "ss4/v1");
        assert_eq!(resolved.queue_cap, 1000);
        assert_eq!(resolved.qos, mqtt_publisher::MqttQos::Qos1);
        assert!(resolved.retain_health);
        assert!(resolved.publish_values);
        assert!(resolved.publish_alarms);
        assert!(resolved.value_kpz_ids.is_empty());
        assert!(resolved.value_group_ids.is_empty());
        assert!(resolved.value_reg_ids.is_empty());
    }

    #[test]
    fn mqtt_config_prefers_env_secrets_and_normalizes_fields() {
        let mut cfg = mqtt_cfg();
        cfg.host = Some(" mqtt.local ".to_string());
        cfg.client_id = Some(" ss4-test ".to_string());
        cfg.username = Some("file-user".to_string());
        cfg.username_env = Some("SS4_MQTT_USER".to_string());
        cfg.password = Some("file-pass".to_string());
        cfg.password_env = Some("SS4_MQTT_PASS".to_string());
        cfg.topic_prefix = Some("/plant/ss4/".to_string());
        cfg.queue_cap = Some(0);
        cfg.qos = Some(2);
        cfg.retain_health = Some(false);
        cfg.publish_values = Some(false);
        cfg.value_kpz_ids = Some(vec![3, 0, -1, 5]);
        cfg.value_group_ids = Some(vec![21]);
        cfg.value_reg_ids = Some(vec![6001, 6002]);

        let resolved = resolve_mqtt_config(&cfg, &|name| match name {
            "SS4_MQTT_USER" => Some(" env-user ".to_string()),
            "SS4_MQTT_PASS" => Some(" env-pass ".to_string()),
            _ => None,
        });

        assert_eq!(resolved.host, "mqtt.local");
        assert_eq!(resolved.client_id, "ss4-test");
        assert_eq!(resolved.username.as_deref(), Some("env-user"));
        assert_eq!(resolved.password.as_deref(), Some("env-pass"));
        assert_eq!(resolved.topic_prefix, "plant/ss4");
        assert_eq!(resolved.queue_cap, 1);
        assert_eq!(resolved.qos, mqtt_publisher::MqttQos::Qos2);
        assert!(!resolved.retain_health);
        assert!(!resolved.publish_values);
        assert!(resolved.publish_alarms);
        assert_eq!(resolved.value_kpz_ids, HashSet::from([3, 5]));
        assert_eq!(resolved.value_group_ids, HashSet::from([21]));
        assert_eq!(resolved.value_reg_ids, HashSet::from([6001, 6002]));
    }

    #[test]
    fn mqtt_config_unknown_qos_falls_back_to_qos1() {
        let mut cfg = mqtt_cfg();
        cfg.qos = Some(7);

        let resolved = resolve_mqtt_config(&cfg, &|_| None);

        assert_eq!(resolved.qos, mqtt_publisher::MqttQos::Qos1);
    }

    fn scheduler_cfg(
        max_inflight: Option<usize>,
        auto_inflight: Option<bool>,
        auto_inflight_max: Option<usize>,
        auto_inflight_backlog_per_slot: Option<usize>,
    ) -> FileSchedulerConfig {
        FileSchedulerConfig {
            pool_size: Some(10),
            tick_ms: Some(250),
            sync_period_sec: Some(2),
            max_queue: Some(100),
            max_inflight,
            auto_inflight,
            auto_inflight_max,
            auto_inflight_backlog_per_slot,
        }
    }

    #[test]
    fn scheduler_auto_inflight_keys_do_not_change_fixed_max_inflight() {
        let file_cfg = scheduler_cfg(Some(4), Some(true), Some(9), Some(1));
        let (cfg, has_auto) = resolve_scheduler_config(Some(&file_cfg), &|_| None);

        assert!(has_auto);
        assert_eq!(cfg.max_inflight, 4);
        assert_eq!(cfg.pool_size, 10);
    }

    #[test]
    fn scheduler_auto_inflight_env_is_reported_but_not_applied() {
        let file_cfg = scheduler_cfg(None, None, None, None);
        let (cfg, has_auto) = resolve_scheduler_config(Some(&file_cfg), &|name| match name {
            "SCHED_MAX_INFLIGHT" => Some("5".to_string()),
            "SCHED_AUTO_INFLIGHT" => Some("true".to_string()),
            "SCHED_AUTO_INFLIGHT_MAX" => Some("9".to_string()),
            _ => None,
        });

        assert!(has_auto);
        assert_eq!(cfg.max_inflight, 5);
    }
}
