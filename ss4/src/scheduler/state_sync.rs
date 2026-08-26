use super::*;

impl SchedulerState {
    /// Создает начальное состояние SchedulerState с ограничениями пула/очереди и значениями runtime-параметров по умолчанию.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `pool_size`: размер пула параллельных воркеров.
    /// - `max_queue`: максимальная длина внутренней очереди заданий.
    /// - `max_inflight`: верхняя граница одновременно выполняемых заданий.
    /// - `no_response_failures`: порог подряд идущих no-response до применения backoff.
    /// - `no_response_backoff_sec`: длительность backoff в секундах при no-response.
    /// # Возвращает
    /// - `SchedulerState`: полностью инициализированное состояние планировщика.
    /// # Пример
    /// - `let state = SchedulerState::new_with_limits(4, 2000, 4, 3, 600);`
    pub(super) fn new_with_limits(
        pool_size: usize,
        max_queue: usize,
        max_inflight: usize,
        no_response_failures: u8,
        no_response_backoff_sec: u64,
    ) -> Self {
        Self {
            pool_size,
            max_queue,
            max_inflight,
            inflight_now: 0,
            dropped_backpressure: 0,
            no_response_failures: no_response_failures.max(1),
            no_response_backoff_sec,
            tasks: HashMap::new(),
            queue: JobQueue::new(),
            obj_by_id: Arc::new(HashMap::new()),
            ip_by_id: Arc::new(HashMap::new()),
            port_by_id: Arc::new(HashMap::new()),
            regs_by_group: Arc::new(HashMap::new()),
            mqtt_reg_meta_by_id: Arc::new(HashMap::new()),
            g_script_by_group: Arc::new(HashMap::new()),
            script_bindings_by_kpz_group: Arc::new(HashMap::new()),
            script_bindings_groups_by_kpz: Arc::new(HashMap::new()),
            script_fallback_bindings_by_group: Arc::new(HashMap::new()),
            script_cache: ScriptCache::new(),
            protocol_generation: 1,
            rv_by_kpz: HashMap::new(),
            rv_dirty: false,
            reg_id_by_addr: Arc::new(HashMap::new()),
            addr_by_reg_id: Arc::new(HashMap::new()),
            read_func_by_addr: Arc::new(HashMap::new()),
            tip_by_reg_id: Arc::new(HashMap::new()),
            force_archive_once_reg_ids: HashSet::new(),
            primed_archive_once_kpz_reg: HashSet::new(),
            primed_archive_once_by_kpz: HashMap::new(),
            relevant_reg_ids_by_kpz: HashMap::new(),
            n_mb_tit_id: None,
            n_mb_reg_id: None,
            primed_kpz: HashSet::new(),
            no_resp_streak_by_kpz: HashMap::new(),
            idx_seen: HashMap::new(),
            idx_seen_by_kpz: HashMap::new(),
            alarms_enabled: false,
            alarm_rules_by_kpz_reg: Arc::new(HashMap::new()),
            alarm_rule_ids_by_kpz: Arc::new(HashMap::new()),
            alarm_runtime: HashMap::new(),
            alarm_runtime_by_kpz: HashMap::new(),
            alarm_notify_by_rule: Arc::new(HashMap::new()),
            next_elam_cleanup: Instant::now(),
            next_metrics_log: Instant::now() + Duration::from_secs(METRICS_EVERY_SEC),
            metrics_jobs_started: 0,
            metrics_jobs_ok: 0,
            metrics_jobs_err: 0,
            metrics_jobs_timeout: 0,
            metrics_lat_le_100_ms: 0,
            metrics_lat_le_300_ms: 0,
            metrics_lat_le_1000_ms: 0,
            metrics_lat_gt_1000_ms: 0,
            metrics_err_windows_streak: 0,
            metrics_p95_warn_ms: DEFAULT_METRICS_P95_WARN_MS,
            metrics_p95_crit_ms: DEFAULT_METRICS_P95_CRIT_MS,
            modbus_a_timeout_ms: DEFAULT_MODBUS_A_TIMEOUT_MS,
            modbus_script_timeout_ms: DEFAULT_MODBUS_SCRIPT_TIMEOUT_MS,
            last_a_glued_status: HashMap::new(),
            last_health_poll_log_at: HashMap::new(),
            last_diag_warn_at: HashMap::new(),
            next_poll_log_cleanup: Instant::now(),
            last_topology_fingerprint: None,
            telegram: None,
            mqtt: None,
        }
    }

    /// Ограничивает частоту записи health-сообщений в poll_log по типу события (`health_ok`, `health_warn`, `health_crit`).
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kind`: тип health-события для ограничения частоты логирования.
    /// - `now`: текущее монотонное время для rate-limit проверок.
    /// # Возвращает
    /// - `bool`: `true`, если можно писать health poll_log сейчас; иначе `false`.
    /// # Пример
    /// - `if state.should_emit_health_poll_log("health_warn", Instant::now()) { /* write poll_log */ }`
    pub(super) fn should_emit_health_poll_log(&mut self, kind: &str, now: Instant) -> bool {
        let min_interval = Duration::from_secs(HEALTH_POLL_LOG_MIN_INTERVAL_SEC);
        let key = kind.to_string();
        match self.last_health_poll_log_at.get(&key).copied() {
            Some(last) if now.duration_since(last) < min_interval => false,
            _ => {
                self.last_health_poll_log_at.insert(key, now);
                true
            }
        }
    }

    /// Ограничивает частоту повторяющихся диагностических WARN/ERROR по произвольному ключу, чтобы не зашумлять логи.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `key`: логический/служебный ключ регистра или индекса.
    /// - `now`: текущее монотонное время для rate-limit проверок.
    /// # Возвращает
    /// - `bool`: `true`, если диагностическое сообщение можно вывести по rate-limit.
    /// # Пример
    /// - `if state.should_emit_diag_warn("sync:script_binding_load_failed", Instant::now()) { /* warn */ }`
    pub(super) fn should_emit_diag_warn(&mut self, key: &str, now: Instant) -> bool {
        let min_interval = Duration::from_secs(DIAG_WARN_MIN_INTERVAL_SEC);
        match self.last_diag_warn_at.get(key).copied() {
            Some(last) if now.duration_since(last) < min_interval => false,
            _ => {
                self.last_diag_warn_at.insert(key.to_string(), now);
                true
            }
        }
    }

    /// Обновляет быстрые runtime-параметры и alarm-состояние без полного reload topology.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `client`: асинхронный клиент PostgreSQL для чтения/записи runtime-данных.
    /// # Возвращает
    /// - `Result<()>`: `Ok`, если runtime cfg и alarm-state обновлены.
    /// # Пример
    /// - `state.sync_runtime_cfg_and_alarm_state(&client).await?;`
    pub(super) async fn sync_runtime_cfg_and_alarm_state(
        &mut self,
        client: &tokio_postgres::Client,
    ) -> Result<()> {
        if let Some(cfg) = load_scheduler_runtime_cfg(client).await? {
            self.no_response_failures = cfg.no_response_failures.max(1);
            self.no_response_backoff_sec = cfg.no_response_backoff_sec.max(1);
            self.metrics_p95_warn_ms = cfg.metrics_p95_warn_ms.max(100);
            self.metrics_p95_crit_ms = cfg.metrics_p95_crit_ms.max(self.metrics_p95_warn_ms);
            self.modbus_a_timeout_ms = cfg.modbus_a_timeout_ms.max(200);
            self.modbus_script_timeout_ms = cfg.modbus_script_timeout_ms.max(200);
        }
        self.reload_alarm_state(client).await?;
        Ok(())
    }

    /// Пересобирает topology/runtime-состояние из БД: KPI/OBJ/REG/script binding и связанные кэши.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `client`: асинхронный клиент PostgreSQL для чтения/записи runtime-данных.
    /// # Возвращает
    /// - `Result<()>`: `Ok`, если topology и задачи планировщика обновлены.
    /// # Пример
    /// - `state.sync_topology_from_db(&client).await?;`
    pub(super) async fn sync_topology_from_db(
        &mut self,
        client: &tokio_postgres::Client,
    ) -> Result<()> {
        let fingerprint = load_topology_fingerprint(client).await?;
        if self.last_topology_fingerprint == Some(fingerprint) {
            tracing::trace!("topology sync skipped: fingerprint unchanged");
            return Ok(());
        }

        let prev = self.last_topology_fingerprint;
        let kpz_changed = prev
            .map(|p| p.kpz_sig != fingerprint.kpz_sig)
            .unwrap_or(true);
        let conn_changed = prev
            .map(|p| {
                p.obj_sig != fingerprint.obj_sig
                    || p.ip_sig != fingerprint.ip_sig
                    || p.port_sig != fingerprint.port_sig
            })
            .unwrap_or(true);
        let protocol_changed = prev
            .map(|p| {
                p.n_mb_sig != fingerprint.n_mb_sig
                    || p.reg_sig != fingerprint.reg_sig
                    || p.g_script_sig != fingerprint.g_script_sig
                    || p.binding_sig != fingerprint.binding_sig
            })
            .unwrap_or(true);

        let kpz_rows = if kpz_changed {
            Some(load_kpz_rows(client).await?)
        } else {
            None
        };
        if conn_changed {
            let obj_rows = load_obj_rows(client).await?;
            let ip_items = load_items(client, "ip").await?;
            let port_items = load_items(client, "port").await?;
            self.reload_connection_topology_from_rows(obj_rows, ip_items, port_items);
        }
        if protocol_changed {
            let n_mb_items = match load_items(client, "n_mb").await {
                Ok(v) => v,
                Err(e) => {
                    if self.should_emit_diag_warn("sync:n_mb_load_failed", Instant::now()) {
                        tracing::warn!(err = %e, "failed to load n_mb items; continuing with empty map");
                    }
                    Vec::new()
                }
            };
            let regs = load_regs(client).await?;
            let g_scripts = load_g_script_rows(client).await?;
            let binding_rows = match load_script_bindings(client).await {
                Ok(v) => v,
                Err(e) => {
                    let emit_warn = self
                        .should_emit_diag_warn("sync:script_binding_load_failed", Instant::now());
                    let script_groups: HashSet<i32> = g_scripts
                        .iter()
                        .filter(|gs| gs.pre_src.as_deref().unwrap_or("").trim().len() >= 3)
                        .map(|gs| gs.grup)
                        .collect();
                    let mut affected_kpz: Vec<i32> = self
                        .tasks
                        .values()
                        .filter(|t| {
                            let groups = decode_groups(&t.kpz.grups);
                            groups.iter().any(|g| script_groups.contains(g))
                        })
                        .map(|t| t.kpz.id)
                        .collect();
                    affected_kpz.sort_unstable();
                    let sample = affected_kpz
                        .iter()
                        .take(20)
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",");
                    let (db_code, db_msg) = match e.downcast_ref::<tokio_postgres::Error>() {
                        Some(pg) => match pg.as_db_error() {
                            Some(db) => (
                                Some(db.code().code().to_string()),
                                Some(db.message().to_string()),
                            ),
                            None => (None, None),
                        },
                        None => (None, None),
                    };
                    if emit_warn {
                        tracing::warn!(
                            err = %e,
                            db_code = db_code.as_deref().unwrap_or("-"),
                            db_msg = db_msg.as_deref().unwrap_or("-"),
                            affected_kpz_count = affected_kpz.len(),
                            affected_kpz_sample = %sample,
                            "failed to load script_binding rows; script bindings disabled"
                        );
                    }
                    Vec::new()
                }
            };
            self.reload_protocol_topology_from_rows(n_mb_items, regs, g_scripts, binding_rows);
        }

        let mut seen = HashMap::new();
        let now = Instant::now();

        for kpz in kpz_rows.unwrap_or_default() {
            let id = kpz.id;
            let kpz_start = kpz.start;
            seen.insert(id, true);

            let group_list = decode_groups(&kpz.grups);
            if group_list.is_empty() {
                continue;
            }
            let group_id = group_list[0];

            let t_a = clamp_period(kpz.t_a);
            let t_script = clamp_period(kpz.t_script);
            let need_on_start;
            let mut need_on_stop = false;

            match self.tasks.get_mut(&id) {
                None => {
                    let period_a = Duration::from_secs(t_a.max(1) as u64);
                    let period_s = Duration::from_secs(t_script.max(1) as u64);
                    self.tasks.insert(
                        id,
                        KpzTask {
                            kpz: kpz.clone(),
                            group_id,
                            generation: 1,
                            next_a: now + phase_offset(period_a, id, 0),
                            next_script: now + phase_offset(period_s, id, 1),
                            busy_a: false,
                            busy_s: false,
                        },
                    );
                    need_on_start = kpz.start == 1;
                }
                Some(t) => {
                    let prev_start = t.kpz.start;
                    let prev_group_id = t.group_id;
                    let prev_t_a = t.kpz.t_a;
                    let prev_t_script = t.kpz.t_script;
                    t.kpz = kpz;
                    t.group_id = group_id;
                    need_on_start = prev_start == 0 && t.kpz.start == 1;
                    need_on_stop = prev_start == 1 && t.kpz.start == 0;
                    if t.kpz.t_a != t_a {
                        t.kpz.t_a = t_a;
                    }
                    if t.kpz.t_script != t_script {
                        t.kpz.t_script = t_script;
                    }
                    if prev_group_id != t.group_id
                        || prev_start != t.kpz.start
                        || prev_t_a != t.kpz.t_a
                        || prev_t_script != t.kpz.t_script
                    {
                        t.generation = t.generation.wrapping_add(1);
                    }
                }
            }

            if need_on_start {
                self.on_kpz_start(id, now);
                self.prime_rv_cache(client, id).await?;
                insert_poll_log(client, Some(id), "start", "scheduler: start=1").await?;
            }
            if need_on_stop {
                self.on_kpz_stop(id);
                insert_poll_log(client, Some(id), "stop", "scheduler: start=0").await?;
            }

            if kpz_start == 1 && !self.rv_by_kpz.contains_key(&id) {
                self.prime_rv_cache(client, id).await?;
            }
        }

        // remove missing
        if kpz_changed {
            let missing: Vec<i32> = self
                .tasks
                .keys()
                .filter(|id| !seen.contains_key(id))
                .cloned()
                .collect();
            for id in missing {
                tracing::info!(kpz_id = id, "kpz removed from scheduler state");
                self.tasks.remove(&id);
                self.rv_by_kpz.remove(&id);
                self.primed_kpz.remove(&id);
                self.no_resp_streak_by_kpz.remove(&id);
                self.idx_seen_by_kpz.remove(&id);
                self.primed_archive_once_by_kpz.remove(&id);
                self.relevant_reg_ids_by_kpz.remove(&id);
            }
        }

        self.rebuild_relevant_reg_ids_by_kpz();
        self.last_topology_fingerprint = Some(fingerprint);

        Ok(())
    }

    #[allow(dead_code)]
    /// Совместимый full-sync wrapper для call sites, которым нужен старый монолитный проход.
    pub(super) async fn sync_from_db(&mut self, client: &tokio_postgres::Client) -> Result<()> {
        self.sync_runtime_cfg_and_alarm_state(client).await?;
        self.sync_topology_from_db(client).await?;
        self.run_retention_cleanups(client).await;
        Ok(())
    }

    #[allow(dead_code)]
    async fn load_sync_rows(
        &mut self,
        client: &tokio_postgres::Client,
    ) -> Result<(
        Vec<KpzRow>,
        Vec<ObjRow>,
        Vec<(i32, String)>,
        Vec<(i32, String)>,
        Vec<(i32, String)>,
        Vec<Reg>,
        Vec<GScriptRow>,
        Vec<ScriptBindingRow>,
    )> {
        let kpz_rows = load_kpz_rows(client).await?;
        let obj_rows = load_obj_rows(client).await?;
        let ip_items = load_items(client, "ip").await?;
        let port_items = load_items(client, "port").await?;
        let n_mb_items = match load_items(client, "n_mb").await {
            Ok(v) => v,
            Err(e) => {
                if self.should_emit_diag_warn("sync:n_mb_load_failed", Instant::now()) {
                    tracing::warn!(err = %e, "failed to load n_mb items; continuing with empty map");
                }
                Vec::new()
            }
        };
        let regs = load_regs(client).await?;
        let g_scripts = load_g_script_rows(client).await?;
        let binding_rows = match load_script_bindings(client).await {
            Ok(v) => v,
            Err(e) => {
                let emit_warn =
                    self.should_emit_diag_warn("sync:script_binding_load_failed", Instant::now());
                let script_groups: HashSet<i32> = g_scripts
                    .iter()
                    .filter(|gs| gs.pre_src.as_deref().unwrap_or("").trim().len() >= 3)
                    .map(|gs| gs.grup)
                    .collect();
                let mut affected_kpz: Vec<i32> = kpz_rows
                    .iter()
                    .filter(|k| {
                        let groups = decode_groups(&k.grups);
                        groups.iter().any(|g| script_groups.contains(g))
                    })
                    .map(|k| k.id)
                    .collect();
                affected_kpz.sort_unstable();
                let sample = affected_kpz
                    .iter()
                    .take(20)
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                let (db_code, db_msg) = match e.downcast_ref::<tokio_postgres::Error>() {
                    Some(pg) => match pg.as_db_error() {
                        Some(db) => (
                            Some(db.code().code().to_string()),
                            Some(db.message().to_string()),
                        ),
                        None => (None, None),
                    },
                    None => (None, None),
                };
                if emit_warn {
                    tracing::warn!(
                        err = %e,
                        db_code = db_code.as_deref().unwrap_or("-"),
                        db_msg = db_msg.as_deref().unwrap_or("-"),
                        affected_kpz_count = affected_kpz.len(),
                        affected_kpz_sample = %sample,
                        "failed to load script_binding rows; script bindings disabled"
                    );
                }
                Vec::new()
            }
        };

        Ok((
            kpz_rows,
            obj_rows,
            ip_items,
            port_items,
            n_mb_items,
            regs,
            g_scripts,
            binding_rows,
        ))
    }

    #[allow(dead_code)]
    fn reload_topology_from_rows(
        &mut self,
        obj_rows: Vec<ObjRow>,
        ip_items: Vec<(i32, String)>,
        port_items: Vec<(i32, String)>,
        n_mb_items: Vec<(i32, String)>,
        regs: Vec<Reg>,
        g_scripts: Vec<GScriptRow>,
        binding_rows: Vec<ScriptBindingRow>,
    ) {
        self.reload_connection_topology_from_rows(obj_rows, ip_items, port_items);
        self.reload_protocol_topology_from_rows(n_mb_items, regs, g_scripts, binding_rows);
    }

    fn reload_connection_topology_from_rows(
        &mut self,
        obj_rows: Vec<ObjRow>,
        ip_items: Vec<(i32, String)>,
        port_items: Vec<(i32, String)>,
    ) {
        self.obj_by_id = Arc::new(obj_rows.into_iter().map(|o| (o.id, o)).collect());
        self.ip_by_id = Arc::new(ip_items.into_iter().collect());
        self.port_by_id = Arc::new(
            port_items
                .into_iter()
                .map(|(id, name)| (id, name.parse::<u16>().unwrap_or(5100)))
                .collect(),
        );
    }

    pub(super) fn reload_protocol_topology_from_rows(
        &mut self,
        n_mb_items: Vec<(i32, String)>,
        regs: Vec<Reg>,
        g_scripts: Vec<GScriptRow>,
        binding_rows: Vec<ScriptBindingRow>,
    ) {
        self.protocol_generation = self.protocol_generation.wrapping_add(1);

        let mut prev_a_en_by_reg_id: HashMap<i32, bool> = HashMap::new();
        for regs in self.regs_by_group.values() {
            for r in regs.iter() {
                prev_a_en_by_reg_id.insert(r.id, r.a_en);
            }
        }

        let mut next_regs_by_group: HashMap<i32, Vec<Reg>> = HashMap::new();
        let mut next_reg_id_by_addr: HashMap<i32, i32> = HashMap::new();
        let mut next_addr_by_reg_id: HashMap<i32, i32> = HashMap::new();
        let mut next_read_func_by_addr: HashMap<i32, u8> = HashMap::new();
        let mut next_tip_by_reg_id: HashMap<i32, i32> = HashMap::new();
        let mut next_mqtt_reg_meta_by_id: HashMap<i32, MqttRegMeta> = HashMap::new();

        self.n_mb_tit_id = n_mb_items
            .iter()
            .find(|(_, name)| name.trim().eq_ignore_ascii_case("TIT"))
            .map(|(id, _)| *id);
        self.n_mb_reg_id = n_mb_items
            .iter()
            .find(|(_, name)| name.trim().eq_ignore_ascii_case("REG"))
            .map(|(id, _)| *id);

        for r in regs {
            next_mqtt_reg_meta_by_id.insert(
                r.id,
                MqttRegMeta {
                    addr: r.addr,
                    name: r.name.clone(),
                    group_id: r.grup,
                },
            );
            if r.a_en {
                let was_enabled = prev_a_en_by_reg_id.get(&r.id).copied().unwrap_or(false);
                if !was_enabled {
                    self.force_archive_once_reg_ids.insert(r.id);
                }
            }
            next_tip_by_reg_id.insert(r.id, r.tip);
            if r.addr >= 0 {
                next_reg_id_by_addr.insert(r.addr, r.id);
                next_addr_by_reg_id.insert(r.id, r.addr);
                if let Some(func) = read_func_for_reg(&r, self.n_mb_tit_id, self.n_mb_reg_id) {
                    next_read_func_by_addr.insert(r.addr, func);
                }
            }
            if let Some(g) = r.grup {
                next_regs_by_group.entry(g).or_default().push(r);
            }
        }
        for regs in next_regs_by_group.values_mut() {
            regs.sort_by_key(|r| r.addr);
        }
        self.regs_by_group = Arc::new(
            next_regs_by_group
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect(),
        );
        self.reg_id_by_addr = Arc::new(next_reg_id_by_addr);
        self.addr_by_reg_id = Arc::new(next_addr_by_reg_id);
        self.read_func_by_addr = Arc::new(next_read_func_by_addr);
        self.tip_by_reg_id = Arc::new(next_tip_by_reg_id);
        self.mqtt_reg_meta_by_id = Arc::new(next_mqtt_reg_meta_by_id);

        self.g_script_by_group = Arc::new(
            g_scripts
                .into_iter()
                .map(|g| (g.grup, Arc::new(g)))
                .collect(),
        );
        self.script_fallback_bindings_by_group = Arc::new(
            self.regs_by_group
                .iter()
                .map(|(group_id, regs)| {
                    (
                        *group_id,
                        Arc::new(
                            regs.iter()
                                .map(|r| RegBinding {
                                    logical: r.id,
                                    reg_id: r.id,
                                    addr: r.addr,
                                })
                                .collect::<Vec<_>>(),
                        ),
                    )
                })
                .collect(),
        );

        let mut next_bindings: HashMap<(i32, i32), Vec<RegBinding>> = HashMap::new();
        let mut next_bindings_by_kpz: HashMap<i32, HashMap<i32, Vec<RegBinding>>> = HashMap::new();
        for b in binding_rows {
            let addr = match b.addr {
                Some(a) => a,
                None => match b
                    .reg_id
                    .and_then(|rid| self.addr_by_reg_id.get(&rid).copied())
                {
                    Some(a) => a,
                    None => continue,
                },
            };
            if addr < 0 {
                continue;
            }
            next_bindings
                .entry((b.kpz_id, b.grup))
                .or_default()
                .push(RegBinding {
                    logical: b.logical,
                    reg_id: b.reg_id.unwrap_or(0),
                    addr,
                });
            next_bindings_by_kpz
                .entry(b.kpz_id)
                .or_default()
                .entry(b.grup)
                .or_default()
                .push(RegBinding {
                    logical: b.logical,
                    reg_id: b.reg_id.unwrap_or(0),
                    addr,
                });
        }
        self.script_bindings_by_kpz_group = Arc::new(
            next_bindings
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect(),
        );
        self.script_bindings_groups_by_kpz = Arc::new(
            next_bindings_by_kpz
                .into_iter()
                .map(|(kpz_id, by_group)| {
                    (
                        kpz_id,
                        Arc::new(
                            by_group
                                .into_iter()
                                .map(|(group_id, bindings)| (group_id, Arc::new(bindings)))
                                .collect(),
                        ),
                    )
                })
                .collect(),
        );
    }

    async fn reload_alarm_state(&mut self, client: &tokio_postgres::Client) -> Result<()> {
        match alarms_schema_present(client).await {
            Ok(true) => {
                let rules = load_alarm_rules(client).await?;
                let db_state = load_alarm_state_map(client).await?;
                let route_rows = match load_alarm_notify_routes(client).await {
                    Ok(v) => v,
                    Err(e) => {
                        if self.should_emit_diag_warn(
                            "sync:alarm_notify_route_load_failed",
                            Instant::now(),
                        ) {
                            tracing::warn!(err = %e, "failed to load alarm notify routes from alarm_rule");
                        }
                        Vec::new()
                    }
                };
                let mut by_key: HashMap<(i32, i32), Vec<AlarmRule>> = HashMap::new();
                let mut rule_ids_by_kpz: HashMap<i32, HashSet<i64>> = HashMap::new();
                let mut present_rule_ids: HashSet<i64> = HashSet::new();
                for r in rules {
                    present_rule_ids.insert(r.id);
                    rule_ids_by_kpz.entry(r.kpz_id).or_default().insert(r.id);
                    by_key.entry((r.kpz_id, r.reg_id)).or_default().push(r);
                }
                for (rule_id, active) in db_state {
                    if !present_rule_ids.contains(&rule_id) {
                        continue;
                    }
                    let st = self.alarm_runtime.entry(rule_id).or_default();
                    st.active = active;
                }
                self.alarm_runtime
                    .retain(|rid, _| present_rule_ids.contains(rid));
                self.rebuild_alarm_runtime_by_kpz();
                let mut notify_by_rule: HashMap<i64, Vec<AlarmNotifyRoute>> = HashMap::new();
                for route in route_rows {
                    if present_rule_ids.contains(&route.rule_id) {
                        notify_by_rule.entry(route.rule_id).or_default().push(route);
                    }
                }
                self.alarm_notify_by_rule = Arc::new(
                    notify_by_rule
                        .into_iter()
                        .map(|(k, v)| (k, Arc::new(v)))
                        .collect(),
                );
                self.alarm_rules_by_kpz_reg =
                    Arc::new(by_key.into_iter().map(|(k, v)| (k, Arc::new(v))).collect());
                self.alarm_rule_ids_by_kpz = Arc::new(
                    rule_ids_by_kpz
                        .into_iter()
                        .map(|(kpz_id, rule_ids)| (kpz_id, Arc::new(rule_ids)))
                        .collect(),
                );
                self.alarms_enabled = true;
            }
            Ok(false) => {
                if self.alarms_enabled {
                    tracing::warn!("alarm tables not found; alarm checks disabled");
                }
                self.alarms_enabled = false;
                self.alarm_rules_by_kpz_reg = Arc::new(HashMap::new());
                self.alarm_rule_ids_by_kpz = Arc::new(HashMap::new());
                self.alarm_runtime.clear();
                self.alarm_runtime_by_kpz.clear();
                self.alarm_notify_by_rule = Arc::new(HashMap::new());
            }
            Err(e) => {
                tracing::warn!(err = %e, "failed to probe alarm tables; alarm checks disabled");
                self.alarms_enabled = false;
                self.alarm_rules_by_kpz_reg = Arc::new(HashMap::new());
                self.alarm_rule_ids_by_kpz = Arc::new(HashMap::new());
                self.alarm_runtime.clear();
                self.alarm_runtime_by_kpz.clear();
                self.alarm_notify_by_rule = Arc::new(HashMap::new());
            }
        }
        Ok(())
    }

    pub(super) async fn run_retention_cleanups(&mut self, client: &tokio_postgres::Client) {
        self.next_elam_cleanup = Instant::now() + Duration::from_secs(ELAM_CLEANUP_EVERY_SEC);
        let mut total_deleted = 0i64;
        let mut batches = 0usize;
        loop {
            batches += 1;
            match delete_elam_older_than_days_batch(
                client,
                ELAM_RETENTION_DAYS,
                ELAM_CLEANUP_BATCH_LIMIT,
            )
            .await
            {
                Ok(cnt) => {
                    total_deleted += cnt;
                    if cnt < ELAM_CLEANUP_BATCH_LIMIT || batches >= ELAM_CLEANUP_MAX_BATCHES {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(err = %e, batches = batches, "elam cleanup batch failed");
                    break;
                }
            }
        }
        tracing::info!(
            deleted = total_deleted,
            batches = batches,
            "elam cleanup batch"
        );

        self.next_poll_log_cleanup =
            Instant::now() + Duration::from_secs(POLL_LOG_CLEANUP_EVERY_SEC);
        let mut total_deleted = 0i64;
        let mut batches = 0usize;
        loop {
            batches += 1;
            match delete_poll_log_older_than_days_batch(
                client,
                POLL_LOG_RETENTION_DAYS,
                POLL_LOG_CLEANUP_BATCH_LIMIT,
            )
            .await
            {
                Ok(cnt) => {
                    total_deleted += cnt;
                    if cnt < POLL_LOG_CLEANUP_BATCH_LIMIT || batches >= POLL_LOG_CLEANUP_MAX_BATCHES
                    {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(err = %e, batches = batches, "poll_log cleanup batch failed");
                    break;
                }
            }
        }
        tracing::info!(
            deleted = total_deleted,
            batches = batches,
            "poll_log cleanup batch"
        );
    }

    /// Обрабатывает переход КПЗ в `start=1`: сбрасывает локальный runtime этого КПЗ, очищает отложенные задания и планирует новый A/script цикл.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `now`: текущее монотонное время для rate-limit проверок.
    /// # Возвращает
    /// - результат согласно типу, указанному в сигнатуре функции.
    /// # Пример
    /// - `state.on_kpz_start(kpz_id, Instant::now());`
    pub(super) fn on_kpz_start(&mut self, kpz_id: i32, now: Instant) {
        if let Some(t) = self.tasks.get_mut(&kpz_id) {
            t.generation = t.generation.wrapping_add(1);
            t.busy_a = false;
            t.busy_s = false;
            let period_a = Duration::from_secs(t.kpz.t_a.max(1) as u64);
            let period_s = Duration::from_secs(t.kpz.t_script.max(1) as u64);
            t.next_a = now + phase_offset(period_a, kpz_id, 0);
            t.next_script = now + phase_offset(period_s, kpz_id, 1);
        }

        // Drop only this KPZ pending jobs; other KPZ queues continue unchanged.
        self.queue.retain(|j| j.kpz_id != kpz_id);

        // Reset only this KPZ runtime state to force fresh A/script cycle.
        self.rv_by_kpz.remove(&kpz_id);
        self.primed_kpz.remove(&kpz_id);
        self.no_resp_streak_by_kpz.remove(&kpz_id);
        self.script_cache.invalidate_kpz(kpz_id);
        self.idx_seen.retain(|k, _| ((*k >> 32) as i32) != kpz_id);
        self.idx_seen_by_kpz.remove(&kpz_id);
        self.primed_archive_once_by_kpz.remove(&kpz_id);

        // Force script re-parse for groups enabled in this KPZ.
        let groups = self
            .tasks
            .get(&kpz_id)
            .map(|t| decode_groups(&t.kpz.grups))
            .unwrap_or_default();
        for g in groups {
            self.script_cache.invalidate_group(g);
        }

        tracing::info!(kpz_id = kpz_id, "kpz started: local runtime reloaded");
    }

    /// Обрабатывает переход КПЗ в `start=0`: снимает busy-флаги, удаляет задания КПЗ из очереди и очищает no-response состояние.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// # Возвращает
    /// - результат согласно типу, указанному в сигнатуре функции.
    /// # Пример
    /// - `state.on_kpz_stop(kpz_id);`
    pub(super) fn on_kpz_stop(&mut self, kpz_id: i32) {
        if let Some(t) = self.tasks.get_mut(&kpz_id) {
            t.generation = t.generation.wrapping_add(1);
            t.busy_a = false;
            t.busy_s = false;
        }
        // Drop only this KPZ pending jobs; other KPZ queues continue unchanged.
        self.queue.retain(|j| j.kpz_id != kpz_id);
        self.primed_kpz.remove(&kpz_id);
        self.no_resp_streak_by_kpz.remove(&kpz_id);
        self.idx_seen_by_kpz.remove(&kpz_id);
        self.primed_archive_once_by_kpz.remove(&kpz_id);
        tracing::info!(kpz_id = kpz_id, "kpz stopped: local pending jobs dropped");
    }

    /// Сбрасывает счетчик подряд идущих no-response ошибок для КПЗ после успешного обмена.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// # Возвращает
    /// - результат согласно типу, указанному в сигнатуре функции.
    /// # Пример
    /// - `state.clear_no_response_streak(kpz_id);`
    #[allow(dead_code)]
    pub(super) fn clear_no_response_streak(&mut self, kpz_id: i32) {
        self.no_resp_streak_by_kpz.remove(&kpz_id);
    }

    /// Увеличивает no-response streak; при достижении порога обнуляет признаки готовности индекса и сдвигает `next_a/next_script` на backoff-интервал.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// # Возвращает
    /// - результат согласно типу, указанному в сигнатуре функции.
    /// # Пример
    /// - `state.mark_no_response_and_backoff(kpz_id);`
    #[allow(dead_code)]
    pub(super) fn mark_no_response_and_backoff(&mut self, kpz_id: i32) {
        let streak = {
            let entry = self
                .no_resp_streak_by_kpz
                .entry(kpz_id)
                .and_modify(|v| *v = v.saturating_add(1))
                .or_insert(1);
            *entry
        };

        if streak < self.no_response_failures {
            return;
        }

        // Force script quality guards to "not ready".
        let idx_addr_raw = 400;
        let slot_raw = idx_addr_raw;
        self.set_rv(kpz_id, svc_key(kpz_id, 60000 + slot_raw * 2), 0.0);
        self.set_rv(kpz_id, svc_key(kpz_id, 80000 + slot_raw), 0.0);
        let idx_addr_legacy = 30401;
        let slot_legacy = idx_addr_legacy - 30001;
        self.set_rv(kpz_id, svc_key(kpz_id, 60000 + slot_legacy * 2), 0.0);
        self.set_rv(kpz_id, svc_key(kpz_id, 80000 + slot_legacy), 0.0);

        for (k, seen) in self.idx_seen.iter_mut() {
            if ((*k >> 32) as i32) == kpz_id {
                seen.samples = 0;
            }
        }
        if let Some(by_kpz_seen) = self.idx_seen_by_kpz.get_mut(&kpz_id) {
            for seen in by_kpz_seen.values_mut() {
                seen.samples = 0;
            }
        }

        let backoff = Duration::from_secs(self.no_response_backoff_sec);
        if let Some(t) = self.tasks.get_mut(&kpz_id) {
            let next = Instant::now() + backoff;
            if t.next_a < next {
                t.next_a = next;
            }
            if t.next_script < next {
                t.next_script = next;
            }
        }

        tracing::warn!(
            kpz_id = kpz_id,
            streak = streak,
            backoff_sec = self.no_response_backoff_sec,
            "kpz no-response threshold reached: quality=0 and polling delayed"
        );
    }
}
