use super::*;
use crate::script_cache::RegBinding;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
struct PreCmd {
    addr_human: i32,
    cnt_words: i32,
}

/// Декодирует набор PRE-команд чтения из script-регистров (`1000 + k*3`), применяя валидацию `enable/addr/count` и лимиты `max_k/max_words`.
fn decode_pre_cmds(regs_out: &HashMap<i32, f64>, max_k: i32, max_words: i32) -> Vec<PreCmd> {
    let mut out = Vec::new();
    for k in 0..max_k {
        let base = 1000 + k * 3;
        let en = regs_out.get(&base).copied().unwrap_or(0.0) as i32;
        if en == 0 {
            continue;
        }
        let adr = regs_out.get(&(base + 1)).copied().unwrap_or(0.0) as i32;
        let cnt = regs_out.get(&(base + 2)).copied().unwrap_or(0.0) as i32;
        if adr <= 0 || cnt <= 0 || cnt > max_words {
            continue;
        }
        out.push(PreCmd {
            addr_human: adr,
            cnt_words: cnt,
        });
    }
    out
}

#[cfg(test)]
impl SchedulerState {
    /// Готовит контекст `rv()` для script-mode по использованным ключам, учитывая bindings `logical -> (addr/reg_id)` и fallback-резолв.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `bindings`: сопоставление logical-ключей скрипта с адресами/регистрами.
    /// - `used_keys`: набор logical-ключей, реально используемых планом скрипта.
    /// # Возвращает
    /// - `HashMap<i32,f64>`: контекст значений для вызовов `rv()` в скрипте.
    /// # Пример
    /// - `let rv_ctx = state.build_script_rv_ctx(kpz_id, &bindings, &used_keys);`
    fn build_script_rv_ctx(
        &self,
        kpz_id: i32,
        bindings: &HashMap<i32, RegBinding>,
        used_keys: &[i32],
    ) -> HashMap<i32, f64> {
        let mut out = HashMap::with_capacity(used_keys.len());
        for logical in used_keys {
            let v = if let Some(b) = bindings.get(logical) {
                self.rv_addr(kpz_id, b.addr)
                    .or_else(|| {
                        if b.reg_id > 0 {
                            Some(self.get_rv(kpz_id, b.reg_id as i64))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| self.get_rv(kpz_id, *logical as i64))
            } else {
                self.get_rv(kpz_id, *logical as i64)
            };
            out.insert(*logical, v);
        }
        out
    }

    /// Выполняет script-mode: PRE -> чтения -> POST, проверяет полноту ответов, применяет write-back и индексы ARX, формирует arx_val и ELAM.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `client`: асинхронный клиент PostgreSQL для чтения/записи runtime-данных.
    /// - `transport`: UDP-транспорт с корреляцией запрос/ответ для обмена с устройством.
    /// - `conn`: сетевые и протокольные параметры подключения КПЗ (ip/port/rtu/modem и т.д.).
    /// # Возвращает
    /// - `Result<()>`: результат script-mode цикла для одного КПЗ.
    /// # Пример
    /// - `state.run_script_mode(&client, &transport, &conn).await?;`
    pub(super) async fn run_script_mode(
        &mut self,
        client: &tokio_postgres::Client,
        transport: &UdpCorrelatedTransport,
        conn: &ConnInfo,
    ) -> Result<()> {
        self.refresh_idx_quality_staleness(conn.kpz_id, now_unix());

        let Some(task) = self.tasks.get(&conn.kpz_id) else {
            tracing::warn!(kpz_id = conn.kpz_id, "script-mode skipped: task not found");
            return Ok(());
        };
        let enabled_groups = decode_groups(&task.kpz.grups);

        // Universal index-quality guard:
        // check readiness across all configured FC4 sources for this KPZ
        // (supports multiple archive/index registers, not only mb=400).
        let mut quality_sources = 0usize;
        let mut quality_ready = false;
        for g in &enabled_groups {
            let Some(regs) = self.regs_by_group.get(g) else {
                continue;
            };
            for r in regs.iter() {
                if !(0..=65535).contains(&r.addr) {
                    continue;
                }
                if read_func_for_reg(r, self.n_mb_tit_id, self.n_mb_reg_id) != Some(4) {
                    continue;
                }
                quality_sources += 1;
                let slot = r.addr;
                let ready = self.rv_svc(conn.kpz_id, 60000 + slot * 2).unwrap_or(0.0);
                let quality = self.rv_svc(conn.kpz_id, 80000 + slot).unwrap_or(0.0);
                if ready >= 1.0 && quality >= 100.0 {
                    quality_ready = true;
                    break;
                }
            }
            if quality_ready {
                break;
            }
        }
        if quality_sources > 0 && !quality_ready {
            tracing::debug!(
                kpz_id = conn.kpz_id,
                quality_sources = quality_sources,
                last_a_status = self
                    .last_a_glued_status
                    .get(&conn.kpz_id)
                    .cloned()
                    .unwrap_or_else(|| "-".to_string()),
                "script-mode skipped: index quality not ready on any FC4 source"
            );
            return Ok(());
        }

        let mut script_groups = Vec::new();
        for g in enabled_groups {
            if let Some(gs) = self.g_script_by_group.get(&g) {
                let pre_len = gs.pre_src.as_deref().unwrap_or("").trim().len();
                if pre_len >= 3 {
                    script_groups.push(g);
                }
            }
        }

        if script_groups.is_empty() {
            tracing::debug!(
                kpz_id = conn.kpz_id,
                "script-mode skipped: no script groups"
            );
            return Ok(());
        }

        self.prime_arx_state(client, conn.kpz_id).await?;

        let hi_lo = true;
        let mut rows: Vec<ArxValRow> = Vec::new();
        let mut elam_rows: Vec<ElamRow> = Vec::new();
        let ts_unix = now_unix();

        for g in &script_groups {
            let Some(gs) = self.g_script_by_group.get(g) else {
                continue;
            };
            let bindings = self
                .script_bindings_by_kpz_group
                .get(&(conn.kpz_id, *g))
                .or_else(|| self.script_fallback_bindings_by_group.get(g))
                .cloned()
                .unwrap_or_else(|| Arc::new(Vec::new()));

            let Some(plan) =
                self.script_cache
                    .get_plan(conn.kpz_id, gs.as_ref(), bindings.as_ref())
            else {
                continue;
            };
            let Some(pre) = plan.template.pre.as_ref() else {
                continue;
            };
            let rv_ctx = self.build_script_rv_ctx(
                conn.kpz_id,
                plan.binding_by_logical.as_ref(),
                plan.template.used_keys.as_ref(),
            );
            let rv_ctx_post = self.build_script_rv_ctx(
                conn.kpz_id,
                plan.binding_by_logical.as_ref(),
                plan.template.used_keys.as_ref(),
            );

            let pre_out = pre.eval_result(
                &[],
                hi_lo,
                &|rid| {
                    rv_ctx
                        .get(&rid)
                        .copied()
                        .unwrap_or_else(|| self.get_rv(conn.kpz_id, rid as i64))
                },
                &|_, _| 0.0,
                Some(&|msg| {
                    tracing::debug!(kpz_id = conn.kpz_id, group_id = *g, msg = %msg, "script PRE");
                }),
                None,
                100000,
            );
            let pre_out = match pre_out {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(kpz_id = conn.kpz_id, group_id = *g, err = %e, "script PRE eval failed");
                    continue;
                }
            };

            let max_k = plan.template.row.max_k.unwrap_or(2).clamp(1, 16);
            let max_words = plan.template.row.max_words.unwrap_or(125).clamp(1, 2500);
            let cmds = decode_pre_cmds(&pre_out.regs, max_k, max_words);
            if cmds.is_empty() {
                tracing::debug!(
                    kpz_id = conn.kpz_id,
                    group_id = *g,
                    "script PRE produced no commands"
                );
                continue;
            }

            let mut reqs: Vec<ReadReq> = Vec::new();
            for c in &cmds {
                // Archive PRE commands are always read as Input registers.
                let func: u8 = 4;
                reqs.push(ReadReq {
                    func,
                    addr_human: c.addr_human,
                    cnt_words: c.cnt_words,
                });
            }
            if reqs.is_empty() {
                tracing::warn!(
                    kpz_id = conn.kpz_id,
                    group_id = *g,
                    "script PRE produced no valid requests"
                );
                continue;
            }

            let exec = self
                .exec_glued_reqs(
                    client,
                    transport,
                    conn,
                    &reqs,
                    self.modbus_script_timeout_ms,
                    *g,
                    "script_mode",
                    &mut elam_rows,
                )
                .await?;
            let multi = exec.multi;
            let dur_ms = exec.dur_ms;
            if multi.results.len() != reqs.len() {
                elam_rows.push(build_elam_summary_row(
                    conn,
                    *g,
                    &multi.request,
                    multi.response.as_deref(),
                    dur_ms,
                    reqs.len(),
                    multi.results.len(),
                ));
                tracing::warn!(
                    kpz_id = conn.kpz_id,
                    group_id = *g,
                    expected = reqs.len(),
                    received = multi.results.len(),
                    status = %multi.status,
                    "script-mode packet dropped: responses count mismatch"
                );
                if let Err(e) = insert_elam_rows(client, &elam_rows).await {
                    tracing::warn!(
                        kpz_id = conn.kpz_id,
                        group_id = *g,
                        rows = elam_rows.len(),
                        err = %e,
                        "failed to batch insert script elam rows"
                    );
                }
                return Err(anyhow::anyhow!(
                    "script responses count mismatch (group {}): received {} expected {}",
                    g,
                    multi.results.len(),
                    reqs.len()
                ));
            }
            if multi.status != "OK" {
                tracing::warn!(kpz_id = conn.kpz_id, group_id = *g, status = %multi.status, "script-mode transport status");
            }

            let svc_recv = svc_key(conn.kpz_id, 20);
            let svc_exp = svc_key(conn.kpz_id, 21);

            for (i, cmd) in cmds.iter().enumerate() {
                let Some(res) = multi.results.get(i) else {
                    break;
                };
                if let Some(row) =
                    build_elam_row(conn, *g, res, &multi.request, dur_ms, &multi.status)
                {
                    elam_rows.push(row);
                }
                let Some(resp) = res.response.as_ref() else {
                    continue;
                };
                let Some(mb) = modbus::extract_modbus_frame(resp) else {
                    continue;
                };

                let words = words_from_modbus_frame(mb, cmd.cnt_words);
                if words.len() != cmd.cnt_words as usize {
                    continue;
                }

                self.set_rv(conn.kpz_id, svc_recv, words.len() as f64);
                self.set_rv(conn.kpz_id, svc_exp, cmd.cnt_words as f64);

                let Some(post) = plan.template.post.as_ref() else {
                    continue;
                };
                let post_out = post.eval_result(
                    &words,
                    false,
                    &|rid| {
                        rv_ctx_post
                            .get(&rid)
                            .copied()
                            .unwrap_or_else(|| self.get_rv(conn.kpz_id, rid as i64))
                    },
                    &|_, _| 0.0,
                    Some(&|msg| {
                        tracing::debug!(kpz_id = conn.kpz_id, group_id = *g, msg = %msg, "script POST");
                    }),
                    None,
                    100000,
                );
                let post_out = match post_out {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(kpz_id = conn.kpz_id, group_id = *g, err = %e, "script POST eval failed");
                        continue;
                    }
                };

                self.apply_write_back(client, conn.kpz_id, &post_out.regs)
                    .await?;

                // map post regs -> arx_val
                for (k, v) in &post_out.regs {
                    let key = *k;

                    if let Some(reg_id) = self.map_reg_id(key) {
                        self.set_rv_reg_value(conn.kpz_id, reg_id, *v);
                        let tip = self.tip_of(reg_id);
                        rows.push(ArxValRow {
                            kpz_id: conn.kpz_id,
                            reg_id,
                            ts_unix,
                            tip,
                            val_num: *v,
                            val_raw: f32_raw(*v),
                        });
                    }

                    self.apply_arx_index_update(client, conn.kpz_id, key, *v as i32)
                        .await?;
                }

                // emits
                for ev in &post_out.emits {
                    let key = ev.reg_id;
                    if let Some(reg_id) = self.map_reg_id(key) {
                        let tip = self.tip_of(reg_id);
                        let emit_ts = normalize_emit_ts_unix(ev.ts, ts_unix);
                        self.set_rv_reg_value(conn.kpz_id, reg_id, ev.value);
                        rows.push(ArxValRow {
                            kpz_id: conn.kpz_id,
                            reg_id,
                            ts_unix: emit_ts,
                            tip,
                            val_num: ev.value,
                            val_raw: f32_raw(ev.value),
                        });
                    }
                }
            }
        }

        if !rows.is_empty() {
            let inserted = insert_arx_val_rows(client, &rows).await?;
            tracing::debug!(
                kpz_id = conn.kpz_id,
                rows = rows.len(),
                inserted = inserted,
                dropped = (rows.len() as i64 - inserted),
                "script-mode arx_val write"
            );
        }
        if !elam_rows.is_empty() {
            if let Err(e) = insert_elam_rows(client, &elam_rows).await {
                tracing::warn!(kpz_id = conn.kpz_id, rows = elam_rows.len(), err = %e, "failed to batch insert script elam rows");
            }
        }
        Ok(())
    }
}

impl WorkerCtx {
    fn build_script_rv_ctx(
        &self,
        kpz_id: i32,
        bindings: &HashMap<i32, RegBinding>,
        used_keys: &[i32],
    ) -> HashMap<i32, f64> {
        let mut out = HashMap::with_capacity(used_keys.len());
        for logical in used_keys {
            let v = if let Some(b) = bindings.get(logical) {
                self.rv_addr(kpz_id, b.addr)
                    .or_else(|| {
                        if b.reg_id > 0 {
                            Some(self.get_rv(kpz_id, b.reg_id as i64))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| self.get_rv(kpz_id, *logical as i64))
            } else {
                self.get_rv(kpz_id, *logical as i64)
            };
            out.insert(*logical, v);
        }
        out
    }

    pub(super) async fn run_script_mode(
        &mut self,
        client: &tokio_postgres::Client,
        transport: &UdpCorrelatedTransport,
        conn: &ConnInfo,
    ) -> Result<()> {
        self.refresh_idx_quality_staleness(conn.kpz_id, now_unix());

        let enabled_groups = decode_groups(&self.runtime.task.kpz.grups);
        let mut quality_sources = 0usize;
        let mut quality_ready = false;
        for g in &enabled_groups {
            let Some(regs) = self.shared.regs_by_group.get(g) else {
                continue;
            };
            for r in regs.iter() {
                if !(0..=65535).contains(&r.addr) {
                    continue;
                }
                if read_func_for_reg(r, self.shared.n_mb_tit_id, self.shared.n_mb_reg_id) != Some(4)
                {
                    continue;
                }
                quality_sources += 1;
                let slot = r.addr;
                let ready = self.rv_svc(conn.kpz_id, 60000 + slot * 2).unwrap_or(0.0);
                let quality = self.rv_svc(conn.kpz_id, 80000 + slot).unwrap_or(0.0);
                if ready >= 1.0 && quality >= 100.0 {
                    quality_ready = true;
                    break;
                }
            }
            if quality_ready {
                break;
            }
        }
        if quality_sources > 0 && !quality_ready {
            tracing::debug!(
                kpz_id = conn.kpz_id,
                quality_sources = quality_sources,
                last_a_status = self
                    .runtime
                    .last_a_status
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                "script-mode skipped: index quality not ready on any FC4 source"
            );
            return Ok(());
        }

        let mut script_groups = Vec::new();
        for g in enabled_groups {
            if let Some(gs) = self.shared.g_script_by_group.get(&g) {
                let pre_len = gs.pre_src.as_deref().unwrap_or("").trim().len();
                if pre_len >= 3 {
                    script_groups.push(g);
                }
            }
        }
        if script_groups.is_empty() {
            tracing::debug!(
                kpz_id = conn.kpz_id,
                "script-mode skipped: no script groups"
            );
            return Ok(());
        }

        self.prime_arx_state(client, conn.kpz_id).await?;

        let hi_lo = true;
        let mut rows: Vec<ArxValRow> = Vec::new();
        let mut elam_rows: Vec<ElamRow> = Vec::new();
        let ts_unix = now_unix();

        for g in &script_groups {
            let Some(gs) = self.shared.g_script_by_group.get(g).cloned() else {
                continue;
            };
            let bindings = self
                .shared
                .script_bindings_by_kpz_group
                .get(&(conn.kpz_id, *g))
                .or_else(|| self.shared.script_fallback_bindings_by_group.get(g))
                .cloned()
                .unwrap_or_else(|| Arc::new(Vec::new()));

            let Some(plan) =
                self.runtime
                    .script_cache
                    .get_plan(conn.kpz_id, gs.as_ref(), bindings.as_ref())
            else {
                continue;
            };
            let Some(pre) = plan.template.pre.as_ref() else {
                continue;
            };
            let rv_ctx = self.build_script_rv_ctx(
                conn.kpz_id,
                plan.binding_by_logical.as_ref(),
                plan.template.used_keys.as_ref(),
            );
            let rv_ctx_post = self.build_script_rv_ctx(
                conn.kpz_id,
                plan.binding_by_logical.as_ref(),
                plan.template.used_keys.as_ref(),
            );

            let pre_out = match pre.eval_result(
                &[],
                hi_lo,
                &|rid| rv_ctx.get(&rid).copied().unwrap_or_else(|| self.get_rv(conn.kpz_id, rid as i64)),
                &|_, _| 0.0,
                Some(&|msg| tracing::debug!(kpz_id = conn.kpz_id, group_id = *g, msg = %msg, "script PRE")),
                None,
                100000,
            ) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(kpz_id = conn.kpz_id, group_id = *g, err = %e, "script PRE eval failed");
                    continue;
                }
            };

            let max_k = plan.template.row.max_k.unwrap_or(2).clamp(1, 16);
            let max_words = plan.template.row.max_words.unwrap_or(125).clamp(1, 2500);
            let cmds = decode_pre_cmds(&pre_out.regs, max_k, max_words);
            if cmds.is_empty() {
                tracing::debug!(
                    kpz_id = conn.kpz_id,
                    group_id = *g,
                    "script PRE produced no commands"
                );
                continue;
            }

            let mut reqs = Vec::new();
            for c in &cmds {
                reqs.push(ReadReq {
                    func: 4,
                    addr_human: c.addr_human,
                    cnt_words: c.cnt_words,
                });
            }
            if reqs.is_empty() {
                tracing::warn!(
                    kpz_id = conn.kpz_id,
                    group_id = *g,
                    "script PRE produced no valid requests"
                );
                continue;
            }

            let exec = self
                .exec_glued_reqs(
                    client,
                    transport,
                    conn,
                    &reqs,
                    self.shared.modbus_script_timeout_ms,
                    *g,
                    "script_mode",
                    &mut elam_rows,
                )
                .await?;
            let multi = exec.multi;
            let dur_ms = exec.dur_ms;
            if multi.results.len() != reqs.len() {
                elam_rows.push(build_elam_summary_row(
                    conn,
                    *g,
                    &multi.request,
                    multi.response.as_deref(),
                    dur_ms,
                    reqs.len(),
                    multi.results.len(),
                ));
                self.db_delta.elam_rows.append(&mut elam_rows);
                return Err(anyhow::anyhow!(
                    "script responses count mismatch (group {}): received {} expected {}",
                    g,
                    multi.results.len(),
                    reqs.len()
                ));
            }

            let svc_recv = svc_key(conn.kpz_id, 20);
            let svc_exp = svc_key(conn.kpz_id, 21);
            for (i, cmd) in cmds.iter().enumerate() {
                let Some(res) = multi.results.get(i) else {
                    break;
                };
                if let Some(row) =
                    build_elam_row(conn, *g, res, &multi.request, dur_ms, &multi.status)
                {
                    elam_rows.push(row);
                }
                let Some(resp) = res.response.as_ref() else {
                    continue;
                };
                let Some(mb) = modbus::extract_modbus_frame(resp) else {
                    continue;
                };
                let words = words_from_modbus_frame(mb, cmd.cnt_words);
                if words.len() != cmd.cnt_words as usize {
                    continue;
                }

                self.set_rv(conn.kpz_id, svc_recv, words.len() as f64);
                self.set_rv(conn.kpz_id, svc_exp, cmd.cnt_words as f64);

                let Some(post) = plan.template.post.as_ref() else {
                    continue;
                };
                let post_out = match post.eval_result(
                    &words,
                    false,
                    &|rid| rv_ctx_post.get(&rid).copied().unwrap_or_else(|| self.get_rv(conn.kpz_id, rid as i64)),
                    &|_, _| 0.0,
                    Some(&|msg| tracing::debug!(kpz_id = conn.kpz_id, group_id = *g, msg = %msg, "script POST")),
                    None,
                    100000,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(kpz_id = conn.kpz_id, group_id = *g, err = %e, "script POST eval failed");
                        continue;
                    }
                };

                self.apply_write_back(client, conn.kpz_id, &post_out.regs)
                    .await?;
                for (k, v) in &post_out.regs {
                    let key = *k;
                    if let Some(reg_id) = self.map_reg_id(key) {
                        self.set_rv_reg_value(conn.kpz_id, reg_id, *v);
                        rows.push(ArxValRow {
                            kpz_id: conn.kpz_id,
                            reg_id,
                            ts_unix,
                            tip: self.tip_of(reg_id),
                            val_num: *v,
                            val_raw: f32_raw(*v),
                        });
                    }
                    self.apply_arx_index_update(client, conn.kpz_id, key, *v as i32)
                        .await?;
                }
                for ev in &post_out.emits {
                    if let Some(reg_id) = self.map_reg_id(ev.reg_id) {
                        self.set_rv_reg_value(conn.kpz_id, reg_id, ev.value);
                        rows.push(ArxValRow {
                            kpz_id: conn.kpz_id,
                            reg_id,
                            ts_unix: normalize_emit_ts_unix(ev.ts, ts_unix),
                            tip: self.tip_of(reg_id),
                            val_num: ev.value,
                            val_raw: f32_raw(ev.value),
                        });
                    }
                }
            }
        }

        if !rows.is_empty() {
            self.db_delta.arx_rows.append(&mut rows);
        }
        if !elam_rows.is_empty() {
            self.db_delta.elam_rows.append(&mut elam_rows);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn decode_pre_cmds_skips_when_enable_zero() {
        let mut regs = HashMap::new();
        regs.insert(1000, 0.0);
        regs.insert(1001, 10.0);
        regs.insert(1002, 5.0);
        let out = decode_pre_cmds(&regs, 8, 125);
        assert!(out.is_empty());
    }

    #[test]
    fn decode_pre_cmds_valid_addr_cnt() {
        let mut regs = HashMap::new();
        regs.insert(1000, 1.0);
        regs.insert(1001, 10.0);
        regs.insert(1002, 5.0);
        let out = decode_pre_cmds(&regs, 8, 125);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].addr_human, 10);
        assert_eq!(out[0].cnt_words, 5);
    }

    #[test]
    fn decode_pre_cmds_rejects_cnt_over_max_words() {
        let mut regs = HashMap::new();
        regs.insert(1000, 1.0);
        regs.insert(1001, 10.0);
        regs.insert(1002, 200.0);
        let out = decode_pre_cmds(&regs, 8, 125);
        assert!(out.is_empty());
    }

    #[test]
    fn decode_pre_cmds_multiple_commands() {
        let mut regs = HashMap::new();
        regs.insert(1000, 1.0);
        regs.insert(1001, 10.0);
        regs.insert(1002, 5.0);
        regs.insert(1003, 1.0);
        regs.insert(1004, 20.0);
        regs.insert(1005, 3.0);
        regs.insert(1006, 1.0);
        regs.insert(1007, 30.0);
        regs.insert(1008, 2.0);
        let out = decode_pre_cmds(&regs, 8, 125);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].addr_human, 10);
        assert_eq!(out[0].cnt_words, 5);
        assert_eq!(out[1].addr_human, 20);
        assert_eq!(out[1].cnt_words, 3);
        assert_eq!(out[2].addr_human, 30);
        assert_eq!(out[2].cnt_words, 2);
    }
}
