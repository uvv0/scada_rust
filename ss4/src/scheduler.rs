use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::sync::watch;
use tokio::time::interval;

use crate::db_queries::{
    alarms_schema_present, delete_elam_older_than_days_batch,
    delete_poll_log_older_than_days_batch, insert_alarm_event_columns, insert_arx_val_rows,
    insert_elam_rows, insert_poll_log, insert_poll_log_columns, load_alarm_notify_routes,
    load_alarm_rules, load_alarm_state_map, load_arx_state_map, load_g_script_rows, load_items,
    load_kpz_rows, load_latest_arx_val_map, load_obj_rows, load_regs, load_scheduler_runtime_cfg,
    load_script_bindings, load_topology_fingerprint, set_arx_last_ind_columns,
    upsert_alarm_state_columns, ElamRow, TopologyFingerprint,
};
#[cfg(test)]
use crate::db_queries::{insert_alarm_event, set_arx_last_ind, upsert_alarm_state};
use crate::modbus;
use crate::modbus_service::{request_reqs_glued, ReadReq};
use crate::mqtt_publisher::{
    MqttAlarmPayload, MqttEvent, MqttHealthPayload, MqttPublisher, MqttValueItem,
};
use crate::reg::Reg;
use crate::script_cache::{RegBinding, ScriptCache};
use crate::telegram_notifier::TelegramNotifier;
use crate::types::{
    AlarmNotifyRoute, AlarmRule, ArxValRow, ConnInfo, GScriptRow, KpzRow, ObjRow, ScriptBindingRow,
};
use crate::udp_transport::UdpCorrelatedTransport;

mod alarm;
mod amode;
mod constants;
mod db_delta;
mod db_writer;
mod merge;
mod metrics;
mod poll_plan;
mod post_cmd;
mod queue;
mod rv_state;
mod smode;
mod state_sync;
mod support;
mod worker;

use constants::*;
use db_delta::{AlarmEventRow, AlarmStateUpdate, ArxStateUpdate, DbDelta, PollLogRow};
use db_writer::DbWriter;

use poll_plan::*;
use post_cmd::{build_post_device_mb, extract_post_device_command, is_post_command_key};
use queue::{Job, JobKind, JobQueue, KpzTask};
use support::*;
// Re-exported for merge/state_sync and other submodules via use super::*
#[allow(unused_imports)]
use worker::{
    AlarmRuntime, IdxSeen, TaskDelta, WorkerCtx, WorkerMerge, WorkerMetricsDelta, WorkerRuntime,
    WorkerRuntimeDelta, WorkerShared,
};

pub struct Scheduler {
    pub pool_size: usize,
    pub tick_ms: u64,
    pub sync_period_sec: u64,
    pub max_queue: usize,
    pub max_inflight: usize,
    pub no_response_failures: u8,
    pub no_response_backoff_sec: u64,
    pub telegram: Option<TelegramNotifier>,
    pub mqtt: Option<MqttPublisher>,
}

impl Scheduler {
    /// Главный цикл планировщика: синхронизирует состояние из БД, ставит задания в очередь, обрабатывает результаты воркеров и пишет health-метрики в poll_log.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `client`: асинхронный клиент PostgreSQL для чтения/записи runtime-данных.
    /// # Возвращает
    /// - `Result<()>`: бесконечный цикл; возвращает `Err` при критической ошибке верхнего уровня.
    /// # Пример
    /// - `scheduler.run(client).await?;`
    pub async fn run(&self, client: tokio_postgres::Client) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = shutdown_tx.send(true);
        });
        self.run_with_shutdown(client, shutdown_rx).await
    }

    pub async fn run_with_shutdown(
        &self,
        client: tokio_postgres::Client,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<()> {
        let client = Arc::new(client);
        let db_writer = DbWriter::start(
            Arc::clone(&client),
            self.max_inflight.max(1).saturating_mul(2),
        );
        let mut tick = interval(Duration::from_millis(self.tick_ms));
        let bind_addr = "0.0.0.0:0"
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid scheduler bind address: {e}"))?;
        let transport = UdpCorrelatedTransport::bind(bind_addr).await?;
        tracing::info!(
            pool_size = self.pool_size,
            tick_ms = self.tick_ms,
            sync_period_sec = self.sync_period_sec,
            max_queue = self.max_queue,
            max_inflight = self.max_inflight,
            no_response_failures = self.no_response_failures,
            no_response_backoff_sec = self.no_response_backoff_sec,
            "scheduler started"
        );

        let mut state = SchedulerState::new_with_limits(
            self.pool_size,
            self.max_queue,
            self.max_inflight,
            self.no_response_failures,
            self.no_response_backoff_sec,
        );
        state.telegram = self.telegram.clone();
        state.mqtt = self.mqtt.clone();
        let mut next_runtime_sync = Instant::now();
        let mut next_topology_sync = Instant::now();
        let mut next_cleanup = Instant::now();
        let mut next_db_writer_log = Instant::now() + Duration::from_secs(60);

        loop {
            if !wait_for_tick_or_shutdown(&mut tick, &mut shutdown_rx).await {
                tracing::info!("shutdown signal received (Ctrl+C), stopping scheduler loop");
                break;
            }
            let now = Instant::now();
            if now >= next_runtime_sync {
                next_runtime_sync = now + Duration::from_secs(self.sync_period_sec);
                state.sync_runtime_cfg_and_alarm_state(&client).await?;
            }
            if now >= next_topology_sync {
                next_topology_sync = now + Duration::from_secs(30);
                state.sync_topology_from_db(&client).await?;
            }
            if now >= next_cleanup {
                next_cleanup = now + Duration::from_secs(ELAM_CLEANUP_EVERY_SEC);
                state.run_retention_cleanups(&client).await;
            }
            if now >= next_db_writer_log {
                next_db_writer_log = now + Duration::from_secs(60);
                db_writer.log_stats();
            }
            state.dispatch_due_work();
            state
                .drain_worker_results_with_db_writer(&client, &transport, &db_writer)
                .await?;
            if let Some((kind, msg)) = state.log_metrics_if_due() {
                state.publish_mqtt_health(&kind, &msg);
                if state.should_emit_health_poll_log(&kind, Instant::now()) {
                    if let Err(e) = insert_poll_log(client.as_ref(), None, &kind, &msg).await {
                        tracing::warn!(err = %e, kind = %kind, "failed to write scheduler health poll_log");
                    }
                }
            }
        }

        db_writer.finish().await?;
        tracing::info!("scheduler stopped");
        Ok(())
    }
}

async fn wait_for_tick_or_shutdown(
    tick: &mut tokio::time::Interval,
    shutdown_rx: &mut watch::Receiver<bool>,
) -> bool {
    if *shutdown_rx.borrow() {
        return false;
    }

    tokio::select! {
        _ = shutdown_rx.changed() => !*shutdown_rx.borrow(),
        _ = tick.tick() => true,
    }
}

struct SchedulerState {
    pool_size: usize,
    max_queue: usize,
    max_inflight: usize,
    inflight_now: usize,
    dropped_backpressure: u64,
    no_response_failures: u8,
    no_response_backoff_sec: u64,
    tasks: HashMap<i32, KpzTask>,
    queue: JobQueue,

    obj_by_id: Arc<HashMap<i32, ObjRow>>,
    ip_by_id: Arc<HashMap<i32, String>>,
    port_by_id: Arc<HashMap<i32, u16>>,
    regs_by_group: Arc<HashMap<i32, Arc<Vec<Reg>>>>,
    mqtt_reg_meta_by_id: Arc<HashMap<i32, MqttRegMeta>>,
    g_script_by_group: Arc<HashMap<i32, Arc<GScriptRow>>>,
    script_bindings_by_kpz_group: Arc<HashMap<(i32, i32), Arc<Vec<RegBinding>>>>,
    script_bindings_groups_by_kpz: Arc<HashMap<i32, Arc<HashMap<i32, Arc<Vec<RegBinding>>>>>>,
    script_fallback_bindings_by_group: Arc<HashMap<i32, Arc<Vec<RegBinding>>>>,
    script_cache: ScriptCache,
    protocol_generation: u64,

    rv_by_kpz: HashMap<i32, Arc<HashMap<i64, f64>>>,
    rv_dirty: bool,
    reg_id_by_addr: Arc<HashMap<i32, i32>>,
    addr_by_reg_id: Arc<HashMap<i32, i32>>,
    read_func_by_addr: Arc<HashMap<i32, u8>>,
    tip_by_reg_id: Arc<HashMap<i32, i32>>,
    force_archive_once_reg_ids: HashSet<i32>,
    primed_archive_once_kpz_reg: HashSet<i64>,
    primed_archive_once_by_kpz: HashMap<i32, HashSet<i64>>,
    relevant_reg_ids_by_kpz: HashMap<i32, Arc<HashSet<i32>>>,
    n_mb_tit_id: Option<i32>,
    n_mb_reg_id: Option<i32>,
    primed_kpz: HashSet<i32>,
    no_resp_streak_by_kpz: HashMap<i32, u8>,
    idx_seen: HashMap<i64, IdxSeen>,
    idx_seen_by_kpz: HashMap<i32, HashMap<i32, IdxSeen>>,
    alarms_enabled: bool,
    alarm_rules_by_kpz_reg: Arc<HashMap<(i32, i32), Arc<Vec<AlarmRule>>>>,
    alarm_rule_ids_by_kpz: Arc<HashMap<i32, Arc<HashSet<i64>>>>,
    alarm_runtime: HashMap<i64, AlarmRuntime>,
    alarm_runtime_by_kpz: HashMap<i32, HashMap<i64, AlarmRuntime>>,
    alarm_notify_by_rule: Arc<HashMap<i64, Arc<Vec<AlarmNotifyRoute>>>>,
    next_elam_cleanup: Instant,
    next_metrics_log: Instant,
    metrics_jobs_started: u64,
    metrics_jobs_ok: u64,
    metrics_jobs_err: u64,
    metrics_jobs_timeout: u64,
    metrics_lat_le_100_ms: u64,
    metrics_lat_le_300_ms: u64,
    metrics_lat_le_1000_ms: u64,
    metrics_lat_gt_1000_ms: u64,
    metrics_err_windows_streak: u32,
    metrics_p95_warn_ms: u64,
    metrics_p95_crit_ms: u64,
    modbus_a_timeout_ms: u64,
    modbus_script_timeout_ms: u64,
    last_a_glued_status: HashMap<i32, String>,
    last_health_poll_log_at: HashMap<String, Instant>,
    last_diag_warn_at: HashMap<String, Instant>,
    next_poll_log_cleanup: Instant,
    last_topology_fingerprint: Option<TopologyFingerprint>,
    telegram: Option<TelegramNotifier>,
    mqtt: Option<MqttPublisher>,
}

#[derive(Clone, Debug)]
struct AlarmTransition {
    rule: AlarmRule,
    event_on: bool,
    value: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PostDeviceCmd {
    func: i32,
    addr: i32,
    value: f64,
}

#[derive(Clone)]
struct MqttRegMeta {
    addr: i32,
    name: String,
    group_id: Option<i32>,
}

#[cfg(test)]
mod tests_async;
#[cfg(test)]
mod tests_core;
