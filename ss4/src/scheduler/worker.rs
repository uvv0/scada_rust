use std::sync::Arc;
use std::time::Instant;

use super::*;
use tokio::task::JoinSet;

// Worker/merge types (moved from scheduler.rs for R2 decomposition)
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct IdxSeen {
    pub last_ts: i64,
    pub samples: u8,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AlarmRuntime {
    pub active: bool,
    pub pending_since: i64,
}

#[derive(Clone, Copy)]
pub(super) struct TaskDelta {
    pub generation: u64,
    pub next_a: Instant,
    pub next_script: Instant,
    pub busy_a: bool,
    pub busy_s: bool,
}

#[derive(Clone, Copy, Default)]
pub(super) struct WorkerMetricsDelta {
    pub jobs_started: u64,
    pub jobs_ok: u64,
    pub jobs_err: u64,
    pub jobs_timeout: u64,
    pub lat_le_100_ms: u64,
    pub lat_le_300_ms: u64,
    pub lat_le_1000_ms: u64,
    pub lat_gt_1000_ms: u64,
}

#[derive(Clone)]
pub(super) struct WorkerRuntimeDelta {
    pub task_delta: Option<TaskDelta>,
    pub protocol_generation: u64,
    pub rv: Option<HashMap<i64, f64>>,
    pub primed: bool,
    pub no_resp_streak: Option<u8>,
    pub idx_seen: Option<HashMap<i64, IdxSeen>>,
    pub alarm_runtime: Option<HashMap<i64, AlarmRuntime>>,
    pub last_a_status: Option<String>,
    pub force_archive_once_reg_ids: HashSet<i32>,
    pub primed_archive_once_kpz_reg: HashSet<i64>,
}

#[derive(Clone)]
pub(super) struct WorkerMerge {
    pub runtime_delta: WorkerRuntimeDelta,
    pub metrics_delta: WorkerMetricsDelta,
    pub db_delta: DbDelta,
    pub script_cache: Option<ScriptCache>,
}

#[derive(Clone)]
pub(super) struct WorkerShared {
    pub obj_by_id: Arc<HashMap<i32, ObjRow>>,
    pub ip_by_id: Arc<HashMap<i32, String>>,
    pub port_by_id: Arc<HashMap<i32, u16>>,
    pub regs_by_group: Arc<HashMap<i32, Arc<Vec<Reg>>>>,
    pub g_script_by_group: Arc<HashMap<i32, Arc<GScriptRow>>>,
    pub script_bindings_by_kpz_group: Arc<HashMap<(i32, i32), Arc<Vec<RegBinding>>>>,
    pub script_fallback_bindings_by_group: Arc<HashMap<i32, Arc<Vec<RegBinding>>>>,
    pub reg_id_by_addr: Arc<HashMap<i32, i32>>,
    pub addr_by_reg_id: Arc<HashMap<i32, i32>>,
    pub tip_by_reg_id: Arc<HashMap<i32, i32>>,
    pub alarm_rules_by_kpz_reg: Arc<HashMap<(i32, i32), Arc<Vec<AlarmRule>>>>,
    pub alarm_rule_ids_by_kpz: Arc<HashMap<i32, Arc<HashSet<i64>>>>,
    pub alarm_notify_by_rule: Arc<HashMap<i64, Arc<Vec<AlarmNotifyRoute>>>>,
    pub n_mb_tit_id: Option<i32>,
    pub n_mb_reg_id: Option<i32>,
    pub alarms_enabled: bool,
    pub no_response_failures: u8,
    pub no_response_backoff_sec: u64,
    pub modbus_a_timeout_ms: u64,
    pub modbus_script_timeout_ms: u64,
    pub telegram: Option<TelegramNotifier>,
}

pub(super) struct WorkerRuntime {
    pub task: KpzTask,
    pub protocol_generation: u64,
    pub script_cache: ScriptCache,
    pub rv: Arc<HashMap<i64, f64>>,
    pub rv_dirty: bool,
    pub primed: bool,
    pub no_resp_streak: Option<u8>,
    pub idx_seen: Arc<HashMap<i32, IdxSeen>>,
    pub idx_seen_dirty: bool,
    pub alarm_runtime: Arc<HashMap<i64, AlarmRuntime>>,
    pub alarm_runtime_dirty: bool,
    pub last_a_status: Option<String>,
    pub force_archive_once_reg_ids: HashSet<i32>,
    pub primed_archive_once_kpz_reg: HashSet<i64>,
}

pub(super) struct WorkerCtx {
    pub kpz_id: i32,
    pub shared: WorkerShared,
    pub runtime: WorkerRuntime,
    pub metrics: WorkerMetricsDelta,
    pub db_delta: DbDelta,
}

impl SchedulerState {
    pub(super) fn rebuild_relevant_reg_ids_by_kpz(&mut self) {
        let mut by_kpz: HashMap<i32, Arc<HashSet<i32>>> = HashMap::new();
        for (kpz_id, task) in &self.tasks {
            let enabled_groups: HashSet<i32> = decode_groups(&task.kpz.grups).into_iter().collect();
            if enabled_groups.is_empty() {
                continue;
            }

            let mut relevant_reg_ids: HashSet<i32> = HashSet::new();
            for g in &enabled_groups {
                if let Some(regs) = self.regs_by_group.get(g) {
                    for r in regs.iter() {
                        relevant_reg_ids.insert(r.id);
                    }
                }
            }
            for bindings in self.script_bindings_for_kpz_groups(*kpz_id, &enabled_groups) {
                for b in bindings.iter() {
                    if b.reg_id > 0 {
                        relevant_reg_ids.insert(b.reg_id);
                    }
                }
            }

            if !relevant_reg_ids.is_empty() {
                by_kpz.insert(*kpz_id, Arc::new(relevant_reg_ids));
            }
        }
        self.relevant_reg_ids_by_kpz = by_kpz;
    }

    pub(super) fn rebuild_alarm_runtime_by_kpz(&mut self) {
        let mut by_kpz: HashMap<i32, HashMap<i64, AlarmRuntime>> = HashMap::new();
        for (kpz_id, rule_ids) in self.alarm_rule_ids_by_kpz.iter() {
            let mut runtime = HashMap::new();
            for rule_id in rule_ids.iter() {
                if let Some(state) = self.alarm_runtime.get(rule_id).copied() {
                    runtime.insert(*rule_id, state);
                }
            }
            if !runtime.is_empty() {
                by_kpz.insert(*kpz_id, runtime);
            }
        }
        self.alarm_runtime_by_kpz = by_kpz;
    }

    pub(super) async fn drain_worker_results_with_db_writer(
        &mut self,
        client: &Arc<tokio_postgres::Client>,
        transport: &UdpCorrelatedTransport,
        db_writer: &DbWriter,
    ) -> Result<()> {
        self.drain_worker_results_inner(client, transport, DbSink::Writer(db_writer))
            .await
    }

    pub(super) fn alarm_rule_ids_for_kpz(&self, kpz_id: i32) -> HashSet<i64> {
        if let Some(rule_ids) = self.alarm_rule_ids_by_kpz.get(&kpz_id) {
            return rule_ids.iter().copied().collect();
        }

        let mut rule_ids = HashSet::new();
        for ((k, _reg_id), rules) in self.alarm_rules_by_kpz_reg.iter() {
            if *k == kpz_id {
                for r in rules.iter() {
                    rule_ids.insert(r.id);
                }
            }
        }
        rule_ids
    }

    fn script_bindings_for_kpz_groups(
        &self,
        kpz_id: i32,
        enabled_groups: &HashSet<i32>,
    ) -> Vec<Arc<Vec<RegBinding>>> {
        if let Some(by_group) = self.script_bindings_groups_by_kpz.get(&kpz_id) {
            return enabled_groups
                .iter()
                .filter_map(|group_id| by_group.get(group_id).cloned())
                .collect();
        }

        self.script_bindings_by_kpz_group
            .iter()
            .filter(|((k, g), _)| *k == kpz_id && enabled_groups.contains(g))
            .map(|(_, bindings)| Arc::clone(bindings))
            .collect()
    }

    /// Параллельно исполняет задания из очереди с ограничением пула, собирает результаты воркеров и сливает изменения обратно в основной state.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `client`: асинхронный клиент PostgreSQL для чтения/записи runtime-данных.
    /// - `transport`: UDP-транспорт с корреляцией запрос/ответ для обмена с устройством.
    /// # Возвращает
    /// - `Result<()>`: очередь обработана, результаты воркеров слиты в основной state.
    /// # Пример
    /// - `state.drain_worker_results(&client, &transport).await?;`
    #[cfg(test)]
    pub(super) async fn drain_worker_results(
        &mut self,
        client: &Arc<tokio_postgres::Client>,
        transport: &UdpCorrelatedTransport,
    ) -> Result<()> {
        self.drain_worker_results_inner(client, transport, DbSink::Direct(client.as_ref()))
            .await
    }

    async fn drain_worker_results_inner(
        &mut self,
        client: &Arc<tokio_postgres::Client>,
        transport: &UdpCorrelatedTransport,
        db_sink: DbSink<'_>,
    ) -> Result<()> {
        if self.queue.is_empty() {
            return Ok(());
        }

        let effective_pool = self.pool_size.min(self.max_inflight.max(1));
        let mut running_kpz: HashSet<i32> = HashSet::with_capacity(effective_pool);
        let mut workers: JoinSet<(i32, Result<()>, WorkerMerge)> = JoinSet::new();

        while workers.len() < effective_pool {
            let Some(job) = self.pop_next_spawnable_job(&running_kpz) else {
                break;
            };
            let kpz_id = job.kpz_id;
            let Some(worker_ctx) = self.build_worker_ctx_for_kpz(kpz_id) else {
                continue;
            };

            running_kpz.insert(kpz_id);
            let client_cl = client.clone();
            let transport_cl: UdpCorrelatedTransport = (*transport).clone();
            workers.spawn(async move {
                let (res, merge) = worker_ctx.run_job(&client_cl, &transport_cl, job).await;
                (kpz_id, res, merge)
            });
            self.inflight_now = running_kpz.len();
        }

        while let Some(joined) = workers.join_next().await {
            let (kpz_id, res, merge) =
                joined.map_err(|e| anyhow::anyhow!("worker task join error: {}", e))?;
            let WorkerMerge {
                runtime_delta,
                metrics_delta,
                db_delta,
                script_cache,
            } = merge;
            running_kpz.remove(&kpz_id);
            self.inflight_now = running_kpz.len();
            self.publish_mqtt_db_delta(&db_delta);
            db_sink.persist(db_delta).await?;
            self.complete_worker_merge(
                kpz_id,
                &res,
                WorkerMerge {
                    runtime_delta,
                    metrics_delta,
                    db_delta: DbDelta::default(),
                    script_cache,
                },
            );

            while workers.len() < effective_pool {
                let Some(job) = self.pop_next_spawnable_job(&running_kpz) else {
                    break;
                };
                let kpz_id = job.kpz_id;
                let Some(worker_ctx) = self.build_worker_ctx_for_kpz(kpz_id) else {
                    continue;
                };

                running_kpz.insert(kpz_id);
                let client_cl = client.clone();
                let transport_cl: UdpCorrelatedTransport = (*transport).clone();
                workers.spawn(async move {
                    let (res, merge) = worker_ctx.run_job(&client_cl, &transport_cl, job).await;
                    (kpz_id, res, merge)
                });
                self.inflight_now = running_kpz.len();
            }
        }
        self.inflight_now = 0;

        Ok(())
    }

    /// Извлекает следующее задание, которое можно стартовать без параллельного конфликта по тому же `kpz_id`.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `running_kpz`: множество КПЗ, уже выполняемых параллельными воркерами.
    /// # Возвращает
    /// - `Option<Job>`: задание, которое можно безопасно запустить.
    /// # Пример
    /// - `let next = state.pop_next_spawnable_job(&running_kpz);`
    pub(super) fn pop_next_spawnable_job(&mut self, running_kpz: &HashSet<i32>) -> Option<Job> {
        self.queue.pop_next_spawnable(running_kpz)
    }

    /// Создает срез состояния SchedulerState для одного КПЗ (данные, кэши, правила, биндинги), который безопасно передается в воркер.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// # Возвращает
    /// - `Option<WorkerCtx>`: локальный worker-state для КПЗ, если задача существует.
    /// # Пример
    /// - `let worker_ctx = state.build_worker_ctx_for_kpz(kpz_id);`
    fn build_worker_ctx_for_kpz(&self, kpz_id: i32) -> Option<WorkerCtx> {
        let task = self.tasks.get(&kpz_id)?.clone();
        let enabled_groups: HashSet<i32> = decode_groups(&task.kpz.grups).into_iter().collect();

        let rv = self
            .rv_by_kpz
            .get(&kpz_id)
            .cloned()
            .unwrap_or_else(|| Arc::new(HashMap::new()));

        let primed = self.primed_kpz.contains(&kpz_id);
        let no_resp_streak = self.no_resp_streak_by_kpz.get(&kpz_id).copied();

        let idx_seen = Arc::new(
            self.idx_seen_by_kpz
                .get(&kpz_id)
                .cloned()
                .unwrap_or_default(),
        );
        let alarm_runtime = Arc::new(
            self.alarm_runtime_by_kpz
                .get(&kpz_id)
                .cloned()
                .unwrap_or_default(),
        );
        let relevant_reg_ids = self
            .relevant_reg_ids_by_kpz
            .get(&kpz_id)
            .cloned()
            .unwrap_or_else(|| Arc::new(HashSet::new()));
        // Keep shared reg_id->tip map so script `emit(ts, reg_id, value)` can archive
        // directly by target reg_id even when that reg belongs to another group.
        let tip_by_reg_id = Arc::clone(&self.tip_by_reg_id);

        let force_archive_once_reg_ids: HashSet<i32> = self
            .force_archive_once_reg_ids
            .iter()
            .filter(|id| relevant_reg_ids.contains(id))
            .copied()
            .collect();

        let primed_archive_once_kpz_reg = self
            .primed_archive_once_by_kpz
            .get(&kpz_id)
            .cloned()
            .unwrap_or_default();

        let last_a_status = self.last_a_glued_status.get(&kpz_id).cloned();
        let worker_script_cache = self.script_cache.clone_for_worker(kpz_id, &enabled_groups);

        Some(WorkerCtx {
            kpz_id,
            shared: WorkerShared {
                obj_by_id: Arc::clone(&self.obj_by_id),
                ip_by_id: Arc::clone(&self.ip_by_id),
                port_by_id: Arc::clone(&self.port_by_id),
                regs_by_group: Arc::clone(&self.regs_by_group),
                g_script_by_group: Arc::clone(&self.g_script_by_group),
                script_bindings_by_kpz_group: Arc::clone(&self.script_bindings_by_kpz_group),
                script_fallback_bindings_by_group: Arc::clone(
                    &self.script_fallback_bindings_by_group,
                ),
                reg_id_by_addr: Arc::clone(&self.reg_id_by_addr),
                addr_by_reg_id: Arc::clone(&self.addr_by_reg_id),
                tip_by_reg_id,
                alarm_rules_by_kpz_reg: Arc::clone(&self.alarm_rules_by_kpz_reg),
                alarm_rule_ids_by_kpz: Arc::clone(&self.alarm_rule_ids_by_kpz),
                alarm_notify_by_rule: Arc::clone(&self.alarm_notify_by_rule),
                n_mb_tit_id: self.n_mb_tit_id,
                n_mb_reg_id: self.n_mb_reg_id,
                alarms_enabled: self.alarms_enabled,
                no_response_failures: self.no_response_failures,
                no_response_backoff_sec: self.no_response_backoff_sec,
                modbus_a_timeout_ms: self.modbus_a_timeout_ms,
                modbus_script_timeout_ms: self.modbus_script_timeout_ms,
                telegram: self.telegram.clone(),
            },
            runtime: WorkerRuntime {
                task,
                protocol_generation: self.protocol_generation,
                script_cache: worker_script_cache,
                rv,
                rv_dirty: false,
                primed,
                no_resp_streak,
                idx_seen,
                idx_seen_dirty: false,
                alarm_runtime,
                alarm_runtime_dirty: false,
                last_a_status,
                force_archive_once_reg_ids,
                primed_archive_once_kpz_reg,
            },
            metrics: WorkerMetricsDelta::default(),
            db_delta: DbDelta::default(),
        })
    }

    /// Преобразует worker-state в компактный `WorkerMerge` для обратного слияния в основной планировщик.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// # Возвращает
    /// - `WorkerMerge`: компактный пакет изменений для обратного merge.
    /// # Пример
    /// - `let merge = worker_state.into_worker_merge(kpz_id);`
    #[cfg(test)]
    pub(super) fn into_worker_merge(self, kpz_id: i32) -> WorkerMerge {
        let allowed_rule_ids = self.alarm_rule_ids_for_kpz(kpz_id);
        let task_delta = self.tasks.get(&kpz_id).map(|task| TaskDelta {
            generation: task.generation,
            next_a: task.next_a,
            next_script: task.next_script,
            busy_a: task.busy_a,
            busy_s: task.busy_s,
        });
        let rv = if self.rv_dirty {
            self.rv_by_kpz.get(&kpz_id).map(|rv| rv.as_ref().clone())
        } else {
            None
        };
        let primed = self.primed_kpz.contains(&kpz_id);
        let no_resp_streak = self.no_resp_streak_by_kpz.get(&kpz_id).copied();
        let idx_seen = Some(
            self.idx_seen
                .into_iter()
                .filter(|(k, _)| ((*k >> 32) as i32) == kpz_id)
                .collect::<HashMap<_, _>>(),
        );
        let alarm_runtime = Some(
            self.alarm_runtime
                .into_iter()
                .filter(|(rid, _)| allowed_rule_ids.contains(rid))
                .collect::<HashMap<_, _>>(),
        );

        WorkerMerge {
            runtime_delta: WorkerRuntimeDelta {
                task_delta,
                protocol_generation: self.protocol_generation,
                rv,
                primed,
                no_resp_streak,
                idx_seen,
                alarm_runtime,
                last_a_status: self.last_a_glued_status.get(&kpz_id).cloned(),
                force_archive_once_reg_ids: self.force_archive_once_reg_ids,
                primed_archive_once_kpz_reg: self.primed_archive_once_kpz_reg,
            },
            metrics_delta: WorkerMetricsDelta {
                jobs_started: self.metrics_jobs_started,
                jobs_ok: self.metrics_jobs_ok,
                jobs_err: self.metrics_jobs_err,
                jobs_timeout: self.metrics_jobs_timeout,
                lat_le_100_ms: self.metrics_lat_le_100_ms,
                lat_le_300_ms: self.metrics_lat_le_300_ms,
                lat_le_1000_ms: self.metrics_lat_le_1000_ms,
                lat_gt_1000_ms: self.metrics_lat_gt_1000_ms,
            },
            db_delta: DbDelta::default(),
            script_cache: if self.script_cache.is_dirty() {
                Some(self.script_cache)
            } else {
                None
            },
        }
    }

    pub(super) fn publish_mqtt_health(&self, kind: &str, msg: &str) {
        let Some(mqtt) = self.mqtt.as_ref() else {
            return;
        };
        mqtt.try_publish(MqttEvent::Health(MqttHealthPayload {
            kind: kind.to_string(),
            message: msg.to_string(),
        }));
    }

    fn publish_mqtt_db_delta(&self, db_delta: &DbDelta) {
        let Some(mqtt) = self.mqtt.as_ref() else {
            return;
        };

        let mut values_by_kpz_ts: HashMap<(i32, i64), Vec<MqttValueItem>> = HashMap::new();
        for row in &db_delta.arx_rows {
            let meta = self.mqtt_reg_meta_by_id.get(&row.reg_id);
            let group_id = meta.and_then(|m| m.group_id);
            if !mqtt.should_publish_value(row.kpz_id, group_id, row.reg_id) {
                continue;
            }
            values_by_kpz_ts
                .entry((row.kpz_id, row.ts_unix))
                .or_default()
                .push(MqttValueItem {
                    reg_id: row.reg_id,
                    addr: meta.map(|m| m.addr),
                    name: meta.map(|m| m.name.clone()),
                    group_id,
                    tip: row.tip,
                    value: row.val_num,
                    quality: "ok",
                });
        }
        for ((kpz_id, ts), values) in values_by_kpz_ts {
            let kpz_name = self
                .tasks
                .get(&kpz_id)
                .and_then(|task| task.kpz.name.clone());
            mqtt.try_publish(MqttEvent::Values {
                kpz_id,
                kpz_name,
                ts,
                values,
            });
        }

        for row in &db_delta.alarm_events {
            mqtt.try_publish(MqttEvent::Alarm(MqttAlarmPayload {
                kpz_id: row.kpz_id,
                reg_id: row.reg_id,
                rule_id: row.rule_id,
                event: row.event,
                value: row.value,
                set_lo: row.set_lo,
                set_hi: row.set_hi,
                severity: row.severity,
                code: row.code.clone(),
                message: row.message.clone(),
            }));
        }
    }
}

enum DbSink<'a> {
    #[cfg(test)]
    Direct(&'a tokio_postgres::Client),
    Writer(&'a DbWriter),
}

impl DbSink<'_> {
    async fn persist(&self, db_delta: DbDelta) -> Result<()> {
        match self {
            #[cfg(test)]
            Self::Direct(client) => super::merge::flush_db_delta(client, &db_delta).await,
            Self::Writer(writer) => writer.enqueue(db_delta).await,
        }
    }
}

impl WorkerCtx {
    fn into_worker_merge(self) -> WorkerMerge {
        let allowed_rule_ids = self
            .shared
            .alarm_rule_ids_by_kpz
            .get(&self.kpz_id)
            .map(|ids| ids.iter().copied().collect::<HashSet<_>>())
            .unwrap_or_default();
        let rv = if self.runtime.rv_dirty {
            Some(Arc::try_unwrap(self.runtime.rv).unwrap_or_else(|rv| rv.as_ref().clone()))
        } else {
            None
        };
        let primed = self.runtime.primed;
        let no_resp_streak = self.runtime.no_resp_streak;
        let idx_seen = if self.runtime.idx_seen_dirty {
            Some(
                Arc::try_unwrap(self.runtime.idx_seen)
                    .unwrap_or_else(|m| m.as_ref().clone())
                    .into_iter()
                    .map(|(addr, seen)| {
                        ((((self.kpz_id as i64) << 32) | (addr as u32 as i64)), seen)
                    })
                    .collect::<HashMap<_, _>>(),
            )
        } else {
            None
        };
        let alarm_runtime = if self.runtime.alarm_runtime_dirty {
            Some(
                Arc::try_unwrap(self.runtime.alarm_runtime)
                    .unwrap_or_else(|m| m.as_ref().clone())
                    .into_iter()
                    .filter(|(rid, _)| allowed_rule_ids.contains(rid))
                    .collect::<HashMap<_, _>>(),
            )
        } else {
            None
        };

        WorkerMerge {
            runtime_delta: WorkerRuntimeDelta {
                task_delta: Some(TaskDelta {
                    generation: self.runtime.task.generation,
                    next_a: self.runtime.task.next_a,
                    next_script: self.runtime.task.next_script,
                    busy_a: self.runtime.task.busy_a,
                    busy_s: self.runtime.task.busy_s,
                }),
                protocol_generation: self.runtime.protocol_generation,
                rv,
                primed,
                no_resp_streak,
                idx_seen,
                alarm_runtime,
                last_a_status: self.runtime.last_a_status,
                force_archive_once_reg_ids: self.runtime.force_archive_once_reg_ids,
                primed_archive_once_kpz_reg: self.runtime.primed_archive_once_kpz_reg,
            },
            metrics_delta: self.metrics,
            db_delta: self.db_delta,
            script_cache: if self.runtime.script_cache.is_dirty() {
                Some(self.runtime.script_cache)
            } else {
                None
            },
        }
    }

    fn record_job_latency(&mut self, elapsed: Duration) {
        let ms = elapsed.as_millis() as u64;
        if ms <= 100 {
            self.metrics.lat_le_100_ms += 1;
        } else if ms <= 300 {
            self.metrics.lat_le_300_ms += 1;
        } else if ms <= 1000 {
            self.metrics.lat_le_1000_ms += 1;
        } else {
            self.metrics.lat_gt_1000_ms += 1;
        }
    }

    fn clear_no_response_streak(&mut self, kpz_id: i32) {
        let _ = kpz_id;
        self.runtime.no_resp_streak = None;
    }

    fn mark_no_response_and_backoff(&mut self, kpz_id: i32) {
        let streak = {
            let next = self.runtime.no_resp_streak.unwrap_or(0).saturating_add(1);
            self.runtime.no_resp_streak = Some(next);
            next
        };
        if streak < self.shared.no_response_failures {
            return;
        }

        self.set_rv(kpz_id, svc_key(kpz_id, 60000 + 400 * 2), 0.0);
        self.set_rv(kpz_id, svc_key(kpz_id, 80000 + 400), 0.0);
        let slot_legacy = 30401 - 30001;
        self.set_rv(kpz_id, svc_key(kpz_id, 60000 + slot_legacy * 2), 0.0);
        self.set_rv(kpz_id, svc_key(kpz_id, 80000 + slot_legacy), 0.0);

        let idx_seen = Arc::make_mut(&mut self.runtime.idx_seen);
        self.runtime.idx_seen_dirty = true;
        for seen in idx_seen.values_mut() {
            seen.samples = 0;
        }

        let next = Instant::now() + Duration::from_secs(self.shared.no_response_backoff_sec);
        if self.runtime.task.next_a < next {
            self.runtime.task.next_a = next;
        }
        if self.runtime.task.next_script < next {
            self.runtime.task.next_script = next;
        }
    }

    /// Выполняет одно задание очереди (A или Script), обновляет метрики успеха/ошибок/таймаута и корректно снимает busy-флаги КПЗ.
    async fn run_job(
        mut self,
        client: &tokio_postgres::Client,
        transport: &UdpCorrelatedTransport,
        job: Job,
    ) -> (Result<()>, WorkerMerge) {
        let kpz_id = job.kpz_id;
        let started_at = Instant::now();
        self.metrics.jobs_started += 1;
        let group_id = self.runtime.task.group_id;
        let kpz = self.runtime.task.kpz.clone();
        let conn = match build_conn(
            &kpz,
            &self.shared.obj_by_id,
            &self.shared.ip_by_id,
            &self.shared.port_by_id,
        ) {
            Ok(c) => c,
            Err(e) => {
                match job.kind {
                    JobKind::A => self.runtime.task.busy_a = false,
                    JobKind::S => self.runtime.task.busy_s = false,
                }
                self.metrics.jobs_err += 1;
                if metrics::is_timeout_error(&e) {
                    self.metrics.jobs_timeout += 1;
                }
                self.record_job_latency(started_at.elapsed());
                let merge = self.into_worker_merge();
                return (Err(e), merge);
            }
        };
        let res = match job.kind {
            JobKind::A => {
                if let Err(e) = self.run_a_mode(client, transport, &conn, group_id).await {
                    tracing::error!(kpz_id = kpz_id, group_id = group_id, err = %e, "A-mode job failed");
                    self.metrics.jobs_err += 1;
                    if metrics::is_timeout_error(&e) {
                        self.metrics.jobs_timeout += 1;
                        self.mark_no_response_and_backoff(kpz_id);
                    }
                    Err(e)
                } else {
                    self.metrics.jobs_ok += 1;
                    self.clear_no_response_streak(kpz_id);
                    Ok(())
                }
            }
            JobKind::S => {
                if let Err(e) = self.run_script_mode(client, transport, &conn).await {
                    tracing::error!(kpz_id = kpz_id, err = %e, "Script job failed");
                    self.metrics.jobs_err += 1;
                    if metrics::is_timeout_error(&e) {
                        self.metrics.jobs_timeout += 1;
                        self.mark_no_response_and_backoff(kpz_id);
                    }
                    Err(e)
                } else {
                    self.metrics.jobs_ok += 1;
                    self.clear_no_response_streak(kpz_id);
                    Ok(())
                }
            }
        };
        match job.kind {
            JobKind::A => self.runtime.task.busy_a = false,
            JobKind::S => self.runtime.task.busy_s = false,
        }
        self.record_job_latency(started_at.elapsed());
        let merge = self.into_worker_merge();
        (res, merge)
    }
}
