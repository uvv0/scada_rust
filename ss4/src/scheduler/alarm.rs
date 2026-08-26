use super::*;
use std::sync::Arc;

#[cfg(test)]
impl SchedulerState {
    /// Проверяет alarm-правила для значения регистра с учетом hysteresis и on/off задержек, пишет состояние/события в БД и возвращает список переходов.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `client`: асинхронный клиент PostgreSQL для чтения/записи runtime-данных.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `reg_id`: идентификатор регистра в БД.
    /// - `value`: новое числовое значение регистра для проверки/обновления.
    /// - `now_ts`: текущее Unix-время для проверки устаревания качества.
    /// # Возвращает
    /// - `Result<Vec<AlarmTransition>>`: список подтвержденных переходов alarm за текущий вызов.
    /// # Пример
    /// - `let tr = state.eval_alarms(&client, kpz_id, reg_id, value, now_unix()).await?;`
    pub(super) async fn eval_alarms(
        &mut self,
        client: &tokio_postgres::Client,
        kpz_id: i32,
        reg_id: i32,
        value: f64,
        now_ts: i64,
    ) -> Result<Vec<AlarmTransition>> {
        let mut transitions: Vec<AlarmTransition> = Vec::new();
        if !self.alarms_enabled {
            return Ok(transitions);
        }
        let Some(rules) = self.alarm_rules_by_kpz_reg.get(&(kpz_id, reg_id)).cloned() else {
            return Ok(transitions);
        };

        for rule in rules.iter().cloned() {
            let desired = alarm_should_be_active(
                &rule,
                value,
                self.alarm_runtime
                    .get(&rule.id)
                    .copied()
                    .unwrap_or_default()
                    .active,
            );
            let state = self.alarm_runtime.entry(rule.id).or_default();
            let by_kpz_alarm_runtime = self.alarm_runtime_by_kpz.entry(kpz_id).or_default();

            if desired == state.active {
                state.pending_since = 0;
                by_kpz_alarm_runtime.insert(rule.id, *state);
                continue;
            }

            if state.pending_since == 0 {
                state.pending_since = now_ts;
                by_kpz_alarm_runtime.insert(rule.id, *state);
                continue;
            }

            let wait = if desired {
                rule.on_delay_sec.max(0) as i64
            } else {
                rule.off_delay_sec.max(0) as i64
            };
            if now_ts - state.pending_since < wait {
                continue;
            }

            state.active = desired;
            state.pending_since = 0;
            by_kpz_alarm_runtime.insert(rule.id, *state);

            upsert_alarm_state(client, rule.id, desired, value).await?;
            let event = if desired { "on" } else { "off" };
            insert_alarm_event(
                client,
                kpz_id,
                reg_id,
                rule.id,
                event,
                value,
                rule.set_lo,
                rule.set_hi,
                rule.severity,
                rule.code.as_deref(),
                rule.message.as_deref(),
            )
            .await?;
            if let Some(tg) = &self.telegram {
                let event = if desired { "ON" } else { "OFF" };
                let msg = format!(
                    "[ALARM {event}] kpz={kpz_id} reg={reg_id} rule={} value={:.3} sev={} code={} msg={}",
                    rule.id,
                    value,
                    rule.severity,
                    rule.code.as_deref().unwrap_or("-"),
                    rule.message.as_deref().unwrap_or("-")
                );
                if let Some(routes) = self.alarm_notify_by_rule.get(&rule.id) {
                    let is_lvl1 = rule.cmp.ends_with("_1");
                    for route in routes.iter() {
                        let event_ok = if desired { route.on_on } else { route.on_off };
                        let threshold_ok = if is_lvl1 {
                            route.thr_lvl1
                        } else {
                            route.thr_main
                        };
                        if event_ok && threshold_ok && !route.chat_id.trim().is_empty() {
                            tg.try_send_to(route.chat_id.clone(), msg.clone());
                        }
                    }
                }
            }
            transitions.push(AlarmTransition {
                rule,
                event_on: desired,
                value,
            });
        }
        Ok(transitions)
    }

    /// Выполняет post-скрипт при срабатывании alarm transition в A-mode: подготавливает RV-контекст события, применяет write-back/команды устройству и формирует arx_val строки.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `client`: асинхронный клиент PostgreSQL для чтения/записи runtime-данных.
    /// - `transport`: UDP-транспорт с корреляцией запрос/ответ для обмена с устройством.
    /// - `conn`: сетевые и протокольные параметры подключения КПЗ (ip/port/rtu/modem и т.д.).
    /// - `group_id`: идентификатор группы регистров/скриптов.
    /// - `ts_unix`: текущее время события в Unix-секундах.
    /// - `tr`: переход alarm-состояния (правило, новое состояние, текущее значение).
    /// - `rows`: буфер строк `arx_val` для накопления перед пакетной записью.
    /// # Возвращает
    /// - `Result<()>`: применяет post-hook alarm и обновления; `Ok`, если обработка завершена.
    /// # Пример
    /// - `state.run_a_mode_alarm_post_hook(&client, &transport, &conn, group_id, ts_unix, tr, &mut rows).await?;`
    pub(super) async fn run_a_mode_alarm_post_hook(
        &mut self,
        client: &tokio_postgres::Client,
        transport: &UdpCorrelatedTransport,
        conn: &ConnInfo,
        group_id: i32,
        ts_unix: i64,
        tr: &AlarmTransition,
        rows: &mut Vec<ArxValRow>,
    ) -> Result<()> {
        let en_post = self
            .tasks
            .get(&conn.kpz_id)
            .map(|t| t.kpz.en_post)
            .unwrap_or(false);
        if !en_post {
            return Ok(());
        }
        let Some(gs) = self
            .g_script_by_group
            .get(&group_id)
            .map(|v| v.as_ref().clone())
        else {
            return Ok(());
        };
        let bindings = self
            .script_bindings_by_kpz_group
            .get(&(conn.kpz_id, group_id))
            .or_else(|| self.script_fallback_bindings_by_group.get(&group_id))
            .cloned()
            .unwrap_or_else(|| Arc::new(Vec::new()));
        let Some(plan) = self
            .script_cache
            .get_plan(conn.kpz_id, &gs, bindings.as_ref())
        else {
            return Ok(());
        };
        let Some(post) = plan.template.post.as_ref() else {
            return Ok(());
        };

        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_EVENT_ON),
            if tr.event_on { 1.0 } else { 0.0 },
        );
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_RULE_ID),
            tr.rule.id as f64,
        );
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_REG_ID),
            tr.rule.reg_id as f64,
        );
        self.set_rv(conn.kpz_id, svc_key(conn.kpz_id, RV_ALARM_VALUE), tr.value);
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_SET_LO),
            tr.rule.set_lo.unwrap_or(0.0),
        );
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_SET_HI),
            tr.rule.set_hi.unwrap_or(0.0),
        );
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_HYST),
            tr.rule.hysteresis,
        );
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_SEVERITY),
            tr.rule.severity as f64,
        );
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_TS_UNIX),
            ts_unix as f64,
        );
        // Script rv() currently takes i32 keys; keep short aliases to avoid i32 overflow
        // for large kpz_id where svc_base+offset exceeds i32::MAX.
        self.set_rv(
            conn.kpz_id,
            RV_ALARM_EVENT_ON as i64,
            if tr.event_on { 1.0 } else { 0.0 },
        );
        self.set_rv(conn.kpz_id, RV_ALARM_RULE_ID as i64, tr.rule.id as f64);
        self.set_rv(conn.kpz_id, RV_ALARM_REG_ID as i64, tr.rule.reg_id as f64);
        self.set_rv(conn.kpz_id, RV_ALARM_VALUE as i64, tr.value);
        self.set_rv(
            conn.kpz_id,
            RV_ALARM_SET_LO as i64,
            tr.rule.set_lo.unwrap_or(0.0),
        );
        self.set_rv(
            conn.kpz_id,
            RV_ALARM_SET_HI as i64,
            tr.rule.set_hi.unwrap_or(0.0),
        );
        self.set_rv(conn.kpz_id, RV_ALARM_HYST as i64, tr.rule.hysteresis);
        self.set_rv(
            conn.kpz_id,
            RV_ALARM_SEVERITY as i64,
            tr.rule.severity as f64,
        );
        self.set_rv(conn.kpz_id, RV_ALARM_TS_UNIX as i64, ts_unix as f64);

        let post_out = post.eval_result(
            &[],
            false,
            &|rid| self.get_rv(conn.kpz_id, rid as i64),
            &|_, _| 0.0,
            Some(&|msg| {
                println!(
                    "[script][A-HOOK][kpz={}][group={}] {}",
                    conn.kpz_id, group_id, msg
                );
            }),
            None,
            100000,
        );
        let post_out = match post_out {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    kpz_id = conn.kpz_id,
                    group_id = group_id,
                    rule_id = tr.rule.id,
                    err = %e,
                    "A-mode alarm post hook eval failed"
                );
                return Ok(());
            }
        };

        self.apply_write_back(client, conn.kpz_id, &post_out.regs)
            .await?;
        self.apply_post_device_command(transport, conn, group_id, &post_out.regs)
            .await?;

        for (k, v) in &post_out.regs {
            let key = *k;
            if is_post_command_key(key) {
                continue;
            }

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

        Ok(())
    }

    /// Извлекает команду управления устройством из post-выхода, формирует Modbus FC5/FC6 кадр, отправляет по UDP и логирует tx/rx/status.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `transport`: UDP-транспорт с корреляцией запрос/ответ для обмена с устройством.
    /// - `conn`: сетевые и протокольные параметры подключения КПЗ (ip/port/rtu/modem и т.д.).
    /// - `group_id`: идентификатор группы регистров/скриптов.
    /// - `out_regs`: карта вычисленных регистров скрипта (ключ -> значение).
    /// # Возвращает
    /// - `Result<()>`: `Ok`, если команда отправлена или корректно пропущена по условиям.
    /// # Пример
    /// - `state.apply_post_device_command(&transport, &conn, group_id, &out_regs).await?;`
    pub(super) async fn apply_post_device_command(
        &self,
        transport: &UdpCorrelatedTransport,
        conn: &ConnInfo,
        group_id: i32,
        out_regs: &HashMap<i32, f64>,
    ) -> Result<()> {
        let Some(cmd) = extract_post_device_command(out_regs) else {
            return Ok(());
        };
        let func = cmd.func;
        let addr = cmd.addr;
        let val = cmd.value;

        let Some((addr_wire, mb)) = build_post_device_mb(conn.rtu as u16, cmd) else {
            if !(0..=65535).contains(&addr) {
                tracing::warn!(
                    kpz_id = conn.kpz_id,
                    group_id = group_id,
                    addr = addr,
                    "post device command skipped: address out of range"
                );
            } else {
                tracing::warn!(
                    kpz_id = conn.kpz_id,
                    group_id = group_id,
                    func = func,
                    "post device command skipped: unsupported func (supported: 5,6)"
                );
            }
            return Ok(());
        };

        let obj = self.obj_by_id.get(&conn.obj_id).cloned().unwrap_or(ObjRow {
            id: conn.obj_id,
            name: None,
            ip: Some(conn.ip.clone()),
            port: Some(conn.port.to_string()),
            kanal: None,
            speed: None,
            stop: None,
            parit: None,
            bit: None,
        });
        let udp = obj_to_udp(&obj);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let packet_id = (now.as_millis() & 0xFF) as u8;
        let dsr = (now.as_micros() & 0xFFFF) as u16;
        let par = modbus::UdpParams {
            kan: udp.get("kanal").copied().unwrap_or(3) as u8,
            speed: udp.get("speed").copied().unwrap_or(8) as u8,
            stop: udp.get("stop").copied().unwrap_or(0) as u8,
            par: udp.get("parit").copied().unwrap_or(2) as u8,
            dsr,
            data: udp.get("bit").copied().unwrap_or(8) as u8,
            rtu: conn.rtu as u16,
            modem: conn.modem as u16,
            port: conn.port,
            ip: conn.ip.clone(),
            packet_id,
            pkt_type: 0,
            ..Default::default()
        };

        let total_len = 22 + mb.len();
        let mut tx: Vec<u8> = Vec::with_capacity(total_len);
        tx.extend_from_slice(&modbus::shab(&par, total_len));
        tx.extend_from_slice(&mb);

        let sw = std::time::Instant::now();
        let rx = transport
            .send(
                &tx,
                &conn.ip,
                conn.port,
                Duration::from_millis(1200),
                false,
                Duration::from_millis(80),
            )
            .await?;
        let duration_ms = sw.elapsed().as_millis() as i32;
        let status = if rx.is_some() { "OK" } else { "TIMEOUT" };
        let tx_hex = hex_preview(&tx, 256);
        let rx_hex = match rx.as_ref() {
            Some(v) => hex_preview(v, 256),
            None => "-".to_string(),
        };

        tracing::info!(
            kpz_id = conn.kpz_id,
            group_id = group_id,
            func = func,
            addr = addr,
            addr_wire = addr_wire,
            value = val,
            status = status,
            duration_ms = duration_ms,
            tx_hex = %tx_hex,
            rx_hex = %rx_hex,
            "post device command"
        );

        Ok(())
    }
}

impl WorkerCtx {
    pub(super) async fn eval_alarms(
        &mut self,
        _client: &tokio_postgres::Client,
        kpz_id: i32,
        reg_id: i32,
        value: f64,
        now_ts: i64,
    ) -> Result<Vec<AlarmTransition>> {
        let mut transitions = Vec::new();
        if !self.shared.alarms_enabled {
            return Ok(transitions);
        }
        let Some(rules) = self
            .shared
            .alarm_rules_by_kpz_reg
            .get(&(kpz_id, reg_id))
            .cloned()
        else {
            return Ok(transitions);
        };

        for rule in rules.iter().cloned() {
            let desired = alarm_should_be_active(
                &rule,
                value,
                self.runtime
                    .alarm_runtime
                    .get(&rule.id)
                    .copied()
                    .unwrap_or_default()
                    .active,
            );
            let alarm_runtime = Arc::make_mut(&mut self.runtime.alarm_runtime);
            let state = alarm_runtime.entry(rule.id).or_default();
            self.runtime.alarm_runtime_dirty = true;
            if desired == state.active {
                state.pending_since = 0;
                continue;
            }
            if state.pending_since == 0 {
                state.pending_since = now_ts;
                continue;
            }
            let wait = if desired {
                rule.on_delay_sec.max(0) as i64
            } else {
                rule.off_delay_sec.max(0) as i64
            };
            if now_ts - state.pending_since < wait {
                continue;
            }

            state.active = desired;
            state.pending_since = 0;
            self.db_delta.alarm_state_updates.push(AlarmStateUpdate {
                rule_id: rule.id,
                active: desired,
                value,
            });
            self.db_delta.alarm_events.push(AlarmEventRow {
                kpz_id,
                reg_id,
                rule_id: rule.id,
                event: if desired { "on" } else { "off" },
                value,
                set_lo: rule.set_lo,
                set_hi: rule.set_hi,
                severity: rule.severity,
                code: rule.code.clone(),
                message: rule.message.clone(),
            });

            if let Some(tg) = &self.shared.telegram {
                let event = if desired { "ON" } else { "OFF" };
                let msg = format!(
                    "[ALARM {event}] kpz={kpz_id} reg={reg_id} rule={} value={:.3} sev={} code={} msg={}",
                    rule.id,
                    value,
                    rule.severity,
                    rule.code.as_deref().unwrap_or("-"),
                    rule.message.as_deref().unwrap_or("-")
                );
                if let Some(routes) = self.shared.alarm_notify_by_rule.get(&rule.id) {
                    let is_lvl1 = rule.cmp.ends_with("_1");
                    for route in routes.iter() {
                        let event_ok = if desired { route.on_on } else { route.on_off };
                        let threshold_ok = if is_lvl1 {
                            route.thr_lvl1
                        } else {
                            route.thr_main
                        };
                        if event_ok && threshold_ok && !route.chat_id.trim().is_empty() {
                            tg.try_send_to(route.chat_id.clone(), msg.clone());
                        }
                    }
                }
            }

            transitions.push(AlarmTransition {
                rule,
                event_on: desired,
                value,
            });
        }
        Ok(transitions)
    }

    pub(super) async fn run_a_mode_alarm_post_hook(
        &mut self,
        client: &tokio_postgres::Client,
        transport: &UdpCorrelatedTransport,
        conn: &ConnInfo,
        group_id: i32,
        ts_unix: i64,
        tr: &AlarmTransition,
        rows: &mut Vec<ArxValRow>,
    ) -> Result<()> {
        if !self.runtime.task.kpz.en_post {
            return Ok(());
        }
        let Some(gs) = self
            .shared
            .g_script_by_group
            .get(&group_id)
            .map(|v| v.as_ref().clone())
        else {
            return Ok(());
        };
        let bindings = self
            .shared
            .script_bindings_by_kpz_group
            .get(&(conn.kpz_id, group_id))
            .or_else(|| self.shared.script_fallback_bindings_by_group.get(&group_id))
            .cloned()
            .unwrap_or_else(|| Arc::new(Vec::new()));
        let Some(plan) = self
            .runtime
            .script_cache
            .get_plan(conn.kpz_id, &gs, bindings.as_ref())
        else {
            return Ok(());
        };
        let Some(post) = plan.template.post.as_ref() else {
            return Ok(());
        };

        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_EVENT_ON),
            if tr.event_on { 1.0 } else { 0.0 },
        );
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_RULE_ID),
            tr.rule.id as f64,
        );
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_REG_ID),
            tr.rule.reg_id as f64,
        );
        self.set_rv(conn.kpz_id, svc_key(conn.kpz_id, RV_ALARM_VALUE), tr.value);
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_SET_LO),
            tr.rule.set_lo.unwrap_or(0.0),
        );
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_SET_HI),
            tr.rule.set_hi.unwrap_or(0.0),
        );
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_HYST),
            tr.rule.hysteresis,
        );
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_SEVERITY),
            tr.rule.severity as f64,
        );
        self.set_rv(
            conn.kpz_id,
            svc_key(conn.kpz_id, RV_ALARM_TS_UNIX),
            ts_unix as f64,
        );
        self.set_rv(
            conn.kpz_id,
            RV_ALARM_EVENT_ON as i64,
            if tr.event_on { 1.0 } else { 0.0 },
        );
        self.set_rv(conn.kpz_id, RV_ALARM_RULE_ID as i64, tr.rule.id as f64);
        self.set_rv(conn.kpz_id, RV_ALARM_REG_ID as i64, tr.rule.reg_id as f64);
        self.set_rv(conn.kpz_id, RV_ALARM_VALUE as i64, tr.value);
        self.set_rv(
            conn.kpz_id,
            RV_ALARM_SET_LO as i64,
            tr.rule.set_lo.unwrap_or(0.0),
        );
        self.set_rv(
            conn.kpz_id,
            RV_ALARM_SET_HI as i64,
            tr.rule.set_hi.unwrap_or(0.0),
        );
        self.set_rv(conn.kpz_id, RV_ALARM_HYST as i64, tr.rule.hysteresis);
        self.set_rv(
            conn.kpz_id,
            RV_ALARM_SEVERITY as i64,
            tr.rule.severity as f64,
        );
        self.set_rv(conn.kpz_id, RV_ALARM_TS_UNIX as i64, ts_unix as f64);

        let post_out = match post.eval_result(
            &[],
            false,
            &|rid| self.get_rv(conn.kpz_id, rid as i64),
            &|_, _| 0.0,
            Some(&|msg| {
                println!(
                    "[script][A-HOOK][kpz={}][group={}] {}",
                    conn.kpz_id, group_id, msg
                );
            }),
            None,
            100000,
        ) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    kpz_id = conn.kpz_id,
                    group_id = group_id,
                    rule_id = tr.rule.id,
                    err = %e,
                    "A-mode alarm post hook eval failed"
                );
                return Ok(());
            }
        };

        self.apply_write_back(client, conn.kpz_id, &post_out.regs)
            .await?;
        self.apply_post_device_command(transport, conn, group_id, &post_out.regs)
            .await?;

        for (k, v) in &post_out.regs {
            let key = *k;
            if is_post_command_key(key) {
                continue;
            }
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
            let key = ev.reg_id;
            if let Some(reg_id) = self.map_reg_id(key) {
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
        Ok(())
    }

    pub(super) async fn apply_post_device_command(
        &self,
        transport: &UdpCorrelatedTransport,
        conn: &ConnInfo,
        group_id: i32,
        out_regs: &HashMap<i32, f64>,
    ) -> Result<()> {
        let Some(cmd) = extract_post_device_command(out_regs) else {
            return Ok(());
        };

        let Some((_addr_wire, mb)) = build_post_device_mb(conn.rtu as u16, cmd) else {
            if !(0..=65535).contains(&cmd.addr) {
                tracing::warn!(
                    kpz_id = conn.kpz_id,
                    group_id = group_id,
                    addr = cmd.addr,
                    "post device command skipped: address out of range"
                );
            } else {
                tracing::warn!(
                    kpz_id = conn.kpz_id,
                    group_id = group_id,
                    func = cmd.func,
                    "post device command skipped: unsupported func (supported: 5,6)"
                );
            }
            return Ok(());
        };

        let obj = self
            .shared
            .obj_by_id
            .get(&conn.obj_id)
            .cloned()
            .unwrap_or(ObjRow {
                id: conn.obj_id,
                name: None,
                ip: Some(conn.ip.clone()),
                port: Some(conn.port.to_string()),
                kanal: None,
                speed: None,
                stop: None,
                parit: None,
                bit: None,
            });
        let udp = obj_to_udp(&obj);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let packet_id = (now.as_millis() & 0xFF) as u8;
        let dsr = (now.as_micros() & 0xFFFF) as u16;
        let par = modbus::UdpParams {
            kan: udp.get("kanal").copied().unwrap_or(3) as u8,
            speed: udp.get("speed").copied().unwrap_or(8) as u8,
            stop: udp.get("stop").copied().unwrap_or(0) as u8,
            par: udp.get("parit").copied().unwrap_or(2) as u8,
            dsr,
            data: udp.get("bit").copied().unwrap_or(8) as u8,
            rtu: conn.rtu as u16,
            modem: conn.modem as u16,
            port: conn.port,
            ip: conn.ip.clone(),
            packet_id,
            pkt_type: 0,
            ..Default::default()
        };

        let total_len = 22 + mb.len();
        let mut tx = Vec::with_capacity(total_len);
        tx.extend_from_slice(&modbus::shab(&par, total_len));
        tx.extend_from_slice(&mb);

        let sw = std::time::Instant::now();
        let rx = transport
            .send(
                &tx,
                &conn.ip,
                conn.port,
                Duration::from_millis(1200),
                false,
                Duration::from_millis(80),
            )
            .await?;
        let duration_ms = sw.elapsed().as_millis() as i32;
        let status = if rx.is_some() { "OK" } else { "TIMEOUT" };
        let tx_hex = hex_preview(&tx, 256);
        let rx_hex = match rx.as_ref() {
            Some(v) => hex_preview(v, 256),
            None => "-".to_string(),
        };

        tracing::info!(
            kpz_id = conn.kpz_id,
            group_id = group_id,
            func = cmd.func,
            addr = cmd.addr,
            value = cmd.value,
            addr_wire = build_post_device_mb(conn.rtu as u16, cmd).map(|(wire, _)| wire).unwrap_or_default(),
            status = status,
            duration_ms = duration_ms,
            tx = %tx_hex,
            rx = %rx_hex,
            "post device command"
        );
        Ok(())
    }
}
