use std::collections::HashSet;

use super::poll_plan::ReadBlock;
use super::*;

#[derive(Clone, Debug)]
pub(super) struct GroupPlan {
    pub group_id: i32,
    pub regs_poll_sorted: Vec<Reg>,
    pub write_ids: HashSet<i32>,
}

#[derive(Clone, Debug)]
pub(super) struct BlockPlan {
    pub group_idx: usize,
    pub block: ReadBlock,
}

#[cfg(test)]
impl SchedulerState {
    /// Выполняет A-mode цикл КПЗ: планирует блоки чтения, отправляет glued-запрос, валидирует `received==expected`, обновляет RV/качество/alarms и пишет arx_val+elam.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `client`: асинхронный клиент PostgreSQL для чтения/записи runtime-данных.
    /// - `transport`: UDP-транспорт с корреляцией запрос/ответ для обмена с устройством.
    /// - `conn`: сетевые и протокольные параметры подключения КПЗ (ip/port/rtu/modem и т.д.).
    /// - `start_group_id`: группа, с которой начинается приоритетный обход в цикле опроса.
    /// # Возвращает
    /// - `Result<()>`: результат A-mode цикла для одного КПЗ.
    /// # Пример
    /// - `state.run_a_mode(&client, &transport, &conn, group_id).await?;`
    pub(super) async fn run_a_mode(
        &mut self,
        client: &tokio_postgres::Client,
        transport: &UdpCorrelatedTransport,
        conn: &ConnInfo,
        start_group_id: i32,
    ) -> Result<()> {
        self.refresh_idx_quality_staleness(conn.kpz_id, now_unix());

        let Some(task) = self.tasks.get(&conn.kpz_id) else {
            tracing::warn!(kpz_id = conn.kpz_id, "A-mode skipped: task not found");
            return Ok(());
        };
        let enabled_groups = decode_groups(&task.kpz.grups);
        if enabled_groups.is_empty() {
            tracing::debug!(kpz_id = conn.kpz_id, "A-mode skipped: no enabled groups");
            return Ok(());
        }

        let ordered_groups = ordered_groups(&enabled_groups, start_group_id);

        let mut a_groups: Vec<i32> = Vec::new();
        let mut script_groups: Vec<i32> = Vec::new();

        for g in ordered_groups {
            if let Some(gs) = self.g_script_by_group.get(&g) {
                let pre_len = gs.pre_src.as_deref().unwrap_or("").trim().len();
                if pre_len >= 3 {
                    script_groups.push(g);
                    continue;
                }
            }
            a_groups.push(g);
        }

        if script_groups.len() > 1 {
            insert_poll_log(
                client,
                Some(conn.kpz_id),
                "script",
                &format!("script groups pending: {}", script_groups.len()),
            )
            .await?;
        }

        let mut group_plans: Vec<GroupPlan> = Vec::new();
        let mut block_plans: Vec<BlockPlan> = Vec::new();
        let mut reqs: Vec<ReadReq> = Vec::new();

        for g in a_groups {
            let Some(regs) = self.regs_by_group.get(&g) else {
                continue;
            };
            let Some((regs_poll_sorted, write_ids, blocks)) =
                plan_group_reads(regs.as_ref(), self.n_mb_tit_id, self.n_mb_reg_id, 120)
            else {
                continue;
            };

            let group_idx = group_plans.len();
            group_plans.push(GroupPlan {
                group_id: g,
                regs_poll_sorted,
                write_ids,
            });

            for b in blocks {
                reqs.push(ReadReq {
                    func: b.func,
                    addr_human: b.adr,
                    cnt_words: b.cnt_words,
                });
                block_plans.push(BlockPlan {
                    group_idx,
                    block: b,
                });
            }
        }

        if reqs.is_empty() {
            tracing::debug!(
                kpz_id = conn.kpz_id,
                "A-mode skipped: no requests after planning"
            );
            return Ok(());
        }

        tracing::debug!(
            kpz_id = conn.kpz_id,
            groups = group_plans.len(),
            reqs = reqs.len(),
            "A-mode request prepared"
        );

        let mut elam_rows: Vec<ElamRow> = Vec::new();
        let exec = self
            .exec_glued_reqs(
                client,
                transport,
                conn,
                &reqs,
                self.modbus_a_timeout_ms,
                0,
                "a_mode",
                &mut elam_rows,
            )
            .await?;
        let multi = exec.multi;
        let dur_ms = exec.dur_ms;
        self.last_a_glued_status
            .insert(conn.kpz_id, multi.status.clone());
        if multi.results.len() != reqs.len() {
            elam_rows.push(build_elam_summary_row(
                conn,
                0,
                &multi.request,
                multi.response.as_deref(),
                dur_ms,
                reqs.len(),
                multi.results.len(),
            ));
            tracing::warn!(
                kpz_id = conn.kpz_id,
                expected = reqs.len(),
                received = multi.results.len(),
                status = %multi.status,
                "A-mode packet dropped: responses count mismatch"
            );
            if let Err(e) = insert_elam_rows(client, &elam_rows).await {
                tracing::warn!(kpz_id = conn.kpz_id, rows = elam_rows.len(), err = %e, "failed to batch insert elam rows");
            }
            return Err(anyhow::anyhow!(
                "responses count mismatch: received {} expected {}",
                multi.results.len(),
                reqs.len()
            ));
        }
        if multi.status != "OK" {
            tracing::warn!(kpz_id = conn.kpz_id, status = %multi.status, "A-mode transport status");
        }

        let ts_unix = now_unix();
        let mut rows: Vec<ArxValRow> = Vec::new();

        let hi_lo = true;

        for (i, plan) in block_plans.iter().enumerate() {
            let Some(res) = multi.results.get(i) else {
                break;
            };
            let group_id = group_plans
                .get(plan.group_idx)
                .map(|g| g.group_id)
                .unwrap_or(0);
            if let Some(row) =
                build_elam_row(conn, group_id, res, &multi.request, dur_ms, &multi.status)
            {
                elam_rows.push(row);
            }
            let Some(resp) = res.response.as_ref() else {
                continue;
            };
            let Some(mb) = modbus::extract_modbus_frame(resp) else {
                continue;
            };

            let words = words_from_modbus_frame(mb, plan.block.cnt_words);
            if words.len() != plan.block.cnt_words as usize {
                continue;
            }

            let gp = &group_plans[plan.group_idx];

            for r in &gp.regs_poll_sorted {
                if read_func_for_reg(r, self.n_mb_tit_id, self.n_mb_reg_id) != Some(plan.block.func)
                {
                    continue;
                }
                let width = if r.is_32() { 2 } else { 1 };
                let r_start = r.addr;
                let r_end = r.addr + width - 1;

                let block_start = plan.block.adr;
                let block_end = plan.block.adr + plan.block.cnt_words - 1;

                if r_end < block_start {
                    continue;
                }
                if r_start > block_end {
                    break;
                }
                if r_start < block_start || r_end > block_end {
                    continue;
                }

                let v = decode_numeric(r, &words, plan.block.adr, plan.block.cnt_words, hi_lo);
                let Some(v) = v else {
                    continue;
                };

                let prev_cached = self.rv_reg_id(conn.kpz_id, r.id);
                self.set_rv_reg_value(conn.kpz_id, r.id, v);
                if plan.block.func == 4 {
                    self.update_idx_quality(conn.kpz_id, r.addr, ts_unix, v);
                }
                let alarm_transitions = self
                    .eval_alarms(client, conn.kpz_id, r.id, v, ts_unix)
                    .await?;
                for tr in &alarm_transitions {
                    self.run_a_mode_alarm_post_hook(
                        client, transport, conn, group_id, ts_unix, tr, &mut rows,
                    )
                    .await?;
                }

                if gp.write_ids.contains(&r.id) {
                    let key = ((conn.kpz_id as i64) << 32) | (r.id as u32 as i64);
                    let first_for_kpz_reg = !self.primed_archive_once_kpz_reg.contains(&key);
                    let force_archive = self.force_archive_once_reg_ids.contains(&r.id);
                    if force_archive || first_for_kpz_reg || value_changed(prev_cached, v) {
                        rows.push(ArxValRow {
                            kpz_id: conn.kpz_id,
                            reg_id: r.id,
                            ts_unix,
                            tip: r.tip,
                            val_num: v,
                            val_raw: f32_raw(v),
                        });
                        self.primed_archive_once_kpz_reg.insert(key);
                        if force_archive {
                            self.force_archive_once_reg_ids.remove(&r.id);
                        }
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
                "A-mode arx_val write"
            );
        }
        if !elam_rows.is_empty() {
            if let Err(e) = insert_elam_rows(client, &elam_rows).await {
                tracing::warn!(kpz_id = conn.kpz_id, rows = elam_rows.len(), err = %e, "failed to batch insert elam rows");
            }
        }
        Ok(())
    }
}

impl WorkerCtx {
    pub(super) async fn run_a_mode(
        &mut self,
        client: &tokio_postgres::Client,
        transport: &UdpCorrelatedTransport,
        conn: &ConnInfo,
        start_group_id: i32,
    ) -> Result<()> {
        self.refresh_idx_quality_staleness(conn.kpz_id, now_unix());

        let enabled_groups = decode_groups(&self.runtime.task.kpz.grups);
        if enabled_groups.is_empty() {
            tracing::debug!(kpz_id = conn.kpz_id, "A-mode skipped: no enabled groups");
            return Ok(());
        }

        let ordered_groups = ordered_groups(&enabled_groups, start_group_id);
        let mut a_groups = Vec::new();
        let mut script_groups = Vec::new();
        for g in ordered_groups {
            if let Some(gs) = self.shared.g_script_by_group.get(&g) {
                if gs.pre_src.as_deref().unwrap_or("").trim().len() >= 3 {
                    script_groups.push(g);
                    continue;
                }
            }
            a_groups.push(g);
        }

        if script_groups.len() > 1 {
            let _ = client;
            self.db_delta.poll_logs.push(PollLogRow {
                kpz_id: Some(conn.kpz_id),
                kind: "script".to_string(),
                msg: format!("script groups pending: {}", script_groups.len()),
            });
        }

        let mut group_plans = Vec::new();
        let mut block_plans = Vec::new();
        let mut reqs = Vec::new();
        for g in a_groups {
            let Some(regs) = self.shared.regs_by_group.get(&g) else {
                continue;
            };
            let Some((regs_poll_sorted, write_ids, blocks)) = plan_group_reads(
                regs.as_ref(),
                self.shared.n_mb_tit_id,
                self.shared.n_mb_reg_id,
                120,
            ) else {
                continue;
            };

            let group_idx = group_plans.len();
            group_plans.push(GroupPlan {
                group_id: g,
                regs_poll_sorted,
                write_ids,
            });
            for b in blocks {
                reqs.push(ReadReq {
                    func: b.func,
                    addr_human: b.adr,
                    cnt_words: b.cnt_words,
                });
                block_plans.push(BlockPlan {
                    group_idx,
                    block: b,
                });
            }
        }

        if reqs.is_empty() {
            tracing::debug!(
                kpz_id = conn.kpz_id,
                "A-mode skipped: no requests after planning"
            );
            return Ok(());
        }

        let mut elam_rows = Vec::new();
        let exec = self
            .exec_glued_reqs(
                client,
                transport,
                conn,
                &reqs,
                self.shared.modbus_a_timeout_ms,
                0,
                "a_mode",
                &mut elam_rows,
            )
            .await?;
        let multi = exec.multi;
        let dur_ms = exec.dur_ms;
        self.runtime.last_a_status = Some(multi.status.clone());
        if multi.results.len() != reqs.len() {
            elam_rows.push(build_elam_summary_row(
                conn,
                0,
                &multi.request,
                multi.response.as_deref(),
                dur_ms,
                reqs.len(),
                multi.results.len(),
            ));
            self.db_delta.elam_rows.append(&mut elam_rows);
            return Err(anyhow::anyhow!(
                "responses count mismatch: received {} expected {}",
                multi.results.len(),
                reqs.len()
            ));
        }

        let ts_unix = now_unix();
        let mut rows = Vec::new();
        let hi_lo = true;
        for (i, plan) in block_plans.iter().enumerate() {
            let Some(res) = multi.results.get(i) else {
                break;
            };
            let group_id = group_plans
                .get(plan.group_idx)
                .map(|g| g.group_id)
                .unwrap_or(0);
            if let Some(row) =
                build_elam_row(conn, group_id, res, &multi.request, dur_ms, &multi.status)
            {
                elam_rows.push(row);
            }
            let Some(resp) = res.response.as_ref() else {
                continue;
            };
            let Some(mb) = modbus::extract_modbus_frame(resp) else {
                continue;
            };
            let words = words_from_modbus_frame(mb, plan.block.cnt_words);
            if words.len() != plan.block.cnt_words as usize {
                continue;
            }

            let gp = &group_plans[plan.group_idx];
            for r in &gp.regs_poll_sorted {
                if read_func_for_reg(r, self.shared.n_mb_tit_id, self.shared.n_mb_reg_id)
                    != Some(plan.block.func)
                {
                    continue;
                }
                let width = if r.is_32() { 2 } else { 1 };
                let r_start = r.addr;
                let r_end = r.addr + width - 1;
                let block_start = plan.block.adr;
                let block_end = plan.block.adr + plan.block.cnt_words - 1;
                if r_end < block_start {
                    continue;
                }
                if r_start > block_end {
                    break;
                }
                if r_start < block_start || r_end > block_end {
                    continue;
                }

                let Some(v) =
                    decode_numeric(r, &words, plan.block.adr, plan.block.cnt_words, hi_lo)
                else {
                    continue;
                };
                let prev_cached = self.rv_reg_id(conn.kpz_id, r.id);
                self.set_rv_reg_value(conn.kpz_id, r.id, v);
                if plan.block.func == 4 {
                    self.update_idx_quality(conn.kpz_id, r.addr, ts_unix, v);
                }
                let alarm_transitions = self
                    .eval_alarms(client, conn.kpz_id, r.id, v, ts_unix)
                    .await?;
                for tr in &alarm_transitions {
                    self.run_a_mode_alarm_post_hook(
                        client, transport, conn, group_id, ts_unix, tr, &mut rows,
                    )
                    .await?;
                }

                let key = ((conn.kpz_id as i64) << 32) | (r.id as u32 as i64);
                let first_for_kpz_reg = !self.runtime.primed_archive_once_kpz_reg.contains(&key);
                let force_archive = self.runtime.force_archive_once_reg_ids.contains(&r.id);
                let changed = value_changed(prev_cached, v);
                if gp.write_ids.contains(&r.id) && (changed || first_for_kpz_reg || force_archive) {
                    rows.push(ArxValRow {
                        kpz_id: conn.kpz_id,
                        reg_id: r.id,
                        ts_unix,
                        tip: r.tip,
                        val_num: v,
                        val_raw: f32_raw(v),
                    });
                    self.runtime.primed_archive_once_kpz_reg.insert(key);
                    if force_archive {
                        self.runtime.force_archive_once_reg_ids.remove(&r.id);
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
