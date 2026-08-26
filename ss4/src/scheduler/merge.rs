use super::*;

impl SchedulerState {
    /// Сливает результат работы воркера в основной state: task-флаги, RV, alarms, метрики, script-cache и флаги архивации.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `worker`: снимок изменений состояния, возвращенный воркером.
    /// # Возвращает
    /// - `()`: применяет изменения worker-state к основному состоянию.
    /// # Пример
    /// - `state.merge_worker_merge(kpz_id, worker_merge);`
    fn merge_worker_merge(&mut self, kpz_id: i32, worker: WorkerMerge) {
        let WorkerMerge {
            runtime_delta,
            metrics_delta,
            db_delta: _,
            script_cache,
        } = worker;
        let WorkerRuntimeDelta {
            task_delta,
            protocol_generation,
            rv,
            primed,
            no_resp_streak,
            idx_seen,
            alarm_runtime,
            last_a_status,
            force_archive_once_reg_ids,
            primed_archive_once_kpz_reg,
        } = runtime_delta;
        let WorkerMetricsDelta {
            jobs_started,
            jobs_ok,
            jobs_err,
            jobs_timeout,
            lat_le_100_ms,
            lat_le_300_ms,
            lat_le_1000_ms,
            lat_gt_1000_ms,
        } = metrics_delta;

        let worker_generation = task_delta.map(|delta| delta.generation);
        let current_generation = self.tasks.get(&kpz_id).map(|task| task.generation);
        let task_generation_matches = worker_generation.is_some()
            && current_generation.is_some()
            && worker_generation == current_generation;
        let protocol_generation_matches = protocol_generation == self.protocol_generation;
        let generation_matches = task_generation_matches && protocol_generation_matches;

        if generation_matches {
            if let Some(script_cache) = script_cache.as_ref() {
                self.script_cache.merge_from(script_cache);
            }

            if let Some(task_delta) = task_delta {
                if let Some(task) = self.tasks.get_mut(&kpz_id) {
                    task.busy_a = task_delta.busy_a;
                    task.busy_s = task_delta.busy_s;
                    task.next_a = task_delta.next_a;
                    task.next_script = task_delta.next_script;
                }
            }

            if let Some(rv) = rv {
                self.rv_by_kpz.insert(kpz_id, Arc::new(rv));
            }

            if primed {
                self.primed_kpz.insert(kpz_id);
            } else {
                self.primed_kpz.remove(&kpz_id);
            }
            if let Some(v) = no_resp_streak {
                self.no_resp_streak_by_kpz.insert(kpz_id, v);
            } else {
                self.no_resp_streak_by_kpz.remove(&kpz_id);
            }

            if let Some(idx_seen) = idx_seen {
                self.idx_seen.retain(|k, _| ((*k >> 32) as i32) != kpz_id);
                for (k, v) in idx_seen {
                    if ((k >> 32) as i32) == kpz_id {
                        self.idx_seen.insert(k, v);
                    }
                }
                let kpz_idx_seen = self
                    .idx_seen
                    .iter()
                    .filter_map(|(k, v)| {
                        if ((*k >> 32) as i32) == kpz_id {
                            Some(((*k & 0xFFFF_FFFF) as u32 as i32, *v))
                        } else {
                            None
                        }
                    })
                    .collect::<HashMap<_, _>>();
                if kpz_idx_seen.is_empty() {
                    self.idx_seen_by_kpz.remove(&kpz_id);
                } else {
                    self.idx_seen_by_kpz.insert(kpz_id, kpz_idx_seen);
                }
            }

            if let Some(alarm_runtime) = alarm_runtime {
                let worker_rule_ids = self.alarm_rule_ids_for_kpz(kpz_id);
                self.alarm_runtime
                    .retain(|rid, _| !worker_rule_ids.contains(rid));
                for (rid, st) in alarm_runtime {
                    self.alarm_runtime.insert(rid, st);
                }
                let kpz_alarm_runtime = worker_rule_ids
                    .iter()
                    .filter_map(|rid| self.alarm_runtime.get(rid).copied().map(|st| (*rid, st)))
                    .collect::<HashMap<_, _>>();
                if kpz_alarm_runtime.is_empty() {
                    self.alarm_runtime_by_kpz.remove(&kpz_id);
                } else {
                    self.alarm_runtime_by_kpz.insert(kpz_id, kpz_alarm_runtime);
                }
            }

            if let Some(status) = last_a_status {
                self.last_a_glued_status.insert(kpz_id, status);
            }
            // Worker path only consumes(force->remove), so merge as monotonic removals.
            self.force_archive_once_reg_ids
                .retain(|id| force_archive_once_reg_ids.contains(id));
            // Worker path only primes(insert), so merge as monotonic union.
            self.primed_archive_once_kpz_reg
                .extend(primed_archive_once_kpz_reg.iter().copied());
            if primed_archive_once_kpz_reg.is_empty() {
                self.primed_archive_once_by_kpz.remove(&kpz_id);
            } else {
                self.primed_archive_once_by_kpz
                    .entry(kpz_id)
                    .or_default()
                    .extend(primed_archive_once_kpz_reg);
            }
        } else if worker_generation.is_some() || current_generation.is_some() {
            let reason = match (task_generation_matches, protocol_generation_matches) {
                (false, false) => "task_generation_mismatch+protocol_generation_mismatch",
                (false, true) => "task_generation_mismatch",
                (true, false) => "protocol_generation_mismatch",
                (true, true) => "unknown",
            };
            tracing::debug!(
                kpz_id = kpz_id,
                reason = reason,
                worker_generation = ?worker_generation,
                current_generation = ?current_generation,
                worker_protocol_generation = protocol_generation,
                current_protocol_generation = self.protocol_generation,
                "dropping stale worker merge"
            );
        }

        self.metrics_jobs_started += jobs_started;
        self.metrics_jobs_ok += jobs_ok;
        self.metrics_jobs_err += jobs_err;
        self.metrics_jobs_timeout += jobs_timeout;
        self.metrics_lat_le_100_ms += lat_le_100_ms;
        self.metrics_lat_le_300_ms += lat_le_300_ms;
        self.metrics_lat_le_1000_ms += lat_le_1000_ms;
        self.metrics_lat_gt_1000_ms += lat_gt_1000_ms;
    }

    /// Логирует ошибку воркера (с rate-limit) и завершает стандартное слияние worker-результата.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `res`: результат одного Modbus-запроса из glued-пакета.
    /// - `worker`: снимок изменений состояния, возвращенный воркером.
    /// # Возвращает
    /// - `()`: логирует ошибку воркера (при наличии) и выполняет merge.
    /// # Пример
    /// - `state.complete_worker_merge(kpz_id, &res, worker_merge);`
    pub(super) fn complete_worker_merge(
        &mut self,
        kpz_id: i32,
        res: &Result<()>,
        worker: WorkerMerge,
    ) {
        if let Err(e) = res {
            let key = format!("worker_fail:{}:{}", kpz_id, e);
            if self.should_emit_diag_warn(&key, Instant::now()) {
                tracing::error!(kpz_id = kpz_id, err = %e, "parallel worker failed");
            }
        }
        self.merge_worker_merge(kpz_id, worker);
    }
}

pub(super) async fn flush_db_delta(
    client: &tokio_postgres::Client,
    db_delta: &DbDelta,
) -> Result<()> {
    if !db_delta.arx_rows.is_empty() {
        let inserted = insert_arx_val_rows(client, &db_delta.arx_rows).await?;
        if inserted != db_delta.arx_rows.len() as i64 {
            tracing::debug!(
                rows = db_delta.arx_rows.len(),
                inserted = inserted,
                dropped = (db_delta.arx_rows.len() as i64 - inserted),
                "worker db delta arx_val write"
            );
        }
    }
    if !db_delta.elam_rows.is_empty() {
        insert_elam_rows(client, &db_delta.elam_rows).await?;
    }
    if !db_delta.poll_logs.is_empty() {
        let mut kpz_id = Vec::with_capacity(db_delta.poll_logs.len());
        let mut kpz_id_is_null = Vec::with_capacity(db_delta.poll_logs.len());
        let mut kind = Vec::with_capacity(db_delta.poll_logs.len());
        let mut msg = Vec::with_capacity(db_delta.poll_logs.len());
        for row in &db_delta.poll_logs {
            match row.kpz_id {
                Some(v) => {
                    kpz_id.push(v);
                    kpz_id_is_null.push(false);
                }
                None => {
                    kpz_id.push(0);
                    kpz_id_is_null.push(true);
                }
            }
            kind.push(row.kind.clone());
            msg.push(row.msg.clone());
        }
        insert_poll_log_columns(client, &kpz_id, &kpz_id_is_null, &kind, &msg).await?;
    }
    if !db_delta.alarm_state_updates.is_empty() {
        let mut rule_id = Vec::with_capacity(db_delta.alarm_state_updates.len());
        let mut active = Vec::with_capacity(db_delta.alarm_state_updates.len());
        let mut value = Vec::with_capacity(db_delta.alarm_state_updates.len());
        for update in &db_delta.alarm_state_updates {
            rule_id.push(update.rule_id);
            active.push(update.active);
            value.push(update.value);
        }
        upsert_alarm_state_columns(client, &rule_id, &active, &value).await?;
    }
    if !db_delta.alarm_events.is_empty() {
        let mut kpz_id = Vec::with_capacity(db_delta.alarm_events.len());
        let mut reg_id = Vec::with_capacity(db_delta.alarm_events.len());
        let mut rule_id = Vec::with_capacity(db_delta.alarm_events.len());
        let mut event = Vec::with_capacity(db_delta.alarm_events.len());
        let mut value = Vec::with_capacity(db_delta.alarm_events.len());
        let mut set_lo = Vec::with_capacity(db_delta.alarm_events.len());
        let mut set_lo_is_null = Vec::with_capacity(db_delta.alarm_events.len());
        let mut set_hi = Vec::with_capacity(db_delta.alarm_events.len());
        let mut set_hi_is_null = Vec::with_capacity(db_delta.alarm_events.len());
        let mut severity = Vec::with_capacity(db_delta.alarm_events.len());
        let mut code = Vec::with_capacity(db_delta.alarm_events.len());
        let mut code_is_null = Vec::with_capacity(db_delta.alarm_events.len());
        let mut message = Vec::with_capacity(db_delta.alarm_events.len());
        let mut message_is_null = Vec::with_capacity(db_delta.alarm_events.len());
        for row in &db_delta.alarm_events {
            kpz_id.push(row.kpz_id);
            reg_id.push(row.reg_id);
            rule_id.push(row.rule_id);
            event.push(row.event.to_string());
            value.push(row.value);
            match row.set_lo {
                Some(v) => {
                    set_lo.push(v);
                    set_lo_is_null.push(false);
                }
                None => {
                    set_lo.push(0.0);
                    set_lo_is_null.push(true);
                }
            }
            match row.set_hi {
                Some(v) => {
                    set_hi.push(v);
                    set_hi_is_null.push(false);
                }
                None => {
                    set_hi.push(0.0);
                    set_hi_is_null.push(true);
                }
            }
            severity.push(row.severity);
            match &row.code {
                Some(v) => {
                    code.push(v.clone());
                    code_is_null.push(false);
                }
                None => {
                    code.push(String::new());
                    code_is_null.push(true);
                }
            }
            match &row.message {
                Some(v) => {
                    message.push(v.clone());
                    message_is_null.push(false);
                }
                None => {
                    message.push(String::new());
                    message_is_null.push(true);
                }
            }
        }
        insert_alarm_event_columns(
            client,
            &kpz_id,
            &reg_id,
            &rule_id,
            &event,
            &value,
            &set_lo,
            &set_lo_is_null,
            &set_hi,
            &set_hi_is_null,
            &severity,
            &code,
            &code_is_null,
            &message,
            &message_is_null,
        )
        .await?;
    }
    if !db_delta.arx_state_updates.is_empty() {
        let mut kpz_id = Vec::with_capacity(db_delta.arx_state_updates.len());
        let mut arx_id = Vec::with_capacity(db_delta.arx_state_updates.len());
        let mut last_ind = Vec::with_capacity(db_delta.arx_state_updates.len());
        for update in &db_delta.arx_state_updates {
            kpz_id.push(update.kpz_id);
            arx_id.push(update.arx_id);
            last_ind.push(update.last_ind);
        }
        set_arx_last_ind_columns(client, &kpz_id, &arx_id, &last_ind).await?;
    }
    Ok(())
}
