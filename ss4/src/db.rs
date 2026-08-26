//! Модуль подключения к PostgreSQL и базовой инициализации схемы runtime-планировщика.

use anyhow::Result;
use tokio_postgres::{Client, NoTls};

#[derive(Clone, Debug)]
/// Параметры подключения к PostgreSQL для runtime-планировщика.
pub struct DbConfig {
    pub host: String,
    pub port: u16,
    pub db: String,
    pub user: String,
    pub pass: String,
}

/// Устанавливает подключение к PostgreSQL и подготавливает таблицу `scheduler_runtime_cfg`.
///
/// # Parameters
/// - `cfg`: хост/порт/база/учетные данные подключения.
///
/// # Returns
/// - `Ok(Client)`: готовый клиент БД с запущенной фоновой задачей обслуживания соединения.
/// - `Err(...)`: ошибка подключения или инициализации схемы.
pub async fn connect(cfg: &DbConfig) -> Result<Client> {
    tracing::info!(
        host = %cfg.host,
        port = cfg.port,
        db = %cfg.db,
        user = %cfg.user,
        "connecting to postgres"
    );
    let conn_str = format!(
        "host={} port={} user={} password={} dbname={}",
        cfg.host, cfg.port, cfg.user, cfg.pass, cfg.db
    );
    let (client, conn) = tokio_postgres::connect(&conn_str, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            tracing::error!("db connection error: {}", e);
        }
    });
    client
        .batch_execute(
            "create table if not exists scheduler_runtime_cfg (
                id bigserial primary key,
                no_response_failures integer not null default 3,
                no_response_backoff_sec bigint not null default 600,
                metrics_p95_warn_ms bigint not null default 1000,
                metrics_p95_crit_ms bigint not null default 3000,
                modbus_a_timeout_ms bigint not null default 6000,
                modbus_script_timeout_ms bigint not null default 6000,
                updated_at timestamptz not null default now()
            );

            alter table scheduler_runtime_cfg add column if not exists metrics_p95_warn_ms bigint not null default 1000;
            alter table scheduler_runtime_cfg add column if not exists metrics_p95_crit_ms bigint not null default 3000;
            alter table scheduler_runtime_cfg add column if not exists modbus_a_timeout_ms bigint not null default 6000;
            alter table scheduler_runtime_cfg add column if not exists modbus_script_timeout_ms bigint not null default 6000;

            insert into scheduler_runtime_cfg (id, no_response_failures, no_response_backoff_sec, metrics_p95_warn_ms, metrics_p95_crit_ms, modbus_a_timeout_ms, modbus_script_timeout_ms)
            values (1, 3, 600, 1000, 3000, 6000, 6000)
            on conflict (id) do nothing;",
        )
        .await?;
    tracing::info!("postgres connected");
    Ok(client)
}
