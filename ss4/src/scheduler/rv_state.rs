use super::*;

impl SchedulerState {
    #[cfg(test)]
    const ENABLE_LEGACY_RV_FALLBACK: bool = true;

    /// Читает значение из runtime-кэша по `reg_id` через отображение `reg_id -> raw mb addr` (источник данных в RV хранится по адресу).
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `reg_id`: идентификатор регистра в БД.
    /// # Возвращает
    /// - `Option<f64>`: значение регистра, если найдено в runtime-кэше.
    /// # Пример
    /// - `let v = state.rv_reg_id(kpz_id, reg_id);`
    #[cfg(test)]
    pub(super) fn rv_reg_id(&self, kpz_id: i32, reg_id: i32) -> Option<f64> {
        // Single-source storage in rv: only by raw mb address.
        // reg_id is resolved through addr mapping for backward script compatibility.
        let addr = *self.addr_by_reg_id.get(&reg_id)?;
        self.rv_by_kpz.get(&kpz_id)?.get(&(addr as i64)).copied()
    }

    /// Читает значение из runtime-кэша по Modbus-адресу с промежуточным резолвом через `reg_id_by_addr`.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `addr_human`: человекочитаемый адрес регистра Modbus.
    /// # Возвращает
    /// - `Option<f64>`: значение по адресу регистра, если есть сопоставление и кэш.
    /// # Пример
    /// - `let v = state.rv_addr(kpz_id, 40001);`
    #[cfg(test)]
    pub(super) fn rv_addr(&self, kpz_id: i32, addr_human: i32) -> Option<f64> {
        let reg_id = *self.reg_id_by_addr.get(&addr_human)?;
        self.rv_reg_id(kpz_id, reg_id)
    }

    /// Читает служебное значение RV по scoped-ключу `svc_key(kpz_id, offset)`.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `offset`: смещение внутри scoped-служебного диапазона ключей RV.
    /// # Возвращает
    /// - `Option<f64>`: служебное scoped-значение, если ключ присутствует.
    /// # Пример
    /// - `let ready = state.rv_svc(kpz_id, 60000);`
    #[cfg(test)]
    pub(super) fn rv_svc(&self, kpz_id: i32, offset: i32) -> Option<f64> {
        let key = svc_key(kpz_id, offset);
        self.rv_by_kpz.get(&kpz_id)?.get(&key).copied()
    }

    /// Читает значение по legacy-ключу старого формата (`-2_000_000_000 + key`) для обратной совместимости скриптов.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `key`: логический/служебный ключ регистра или индекса.
    /// # Возвращает
    /// - `Option<f64>`: значение по legacy-ключу, если найдено.
    /// # Пример
    /// - `let v = state.rv_legacy(kpz_id, 71001);`
    #[cfg(test)]
    pub(super) fn rv_legacy(&self, kpz_id: i32, key: i64) -> Option<f64> {
        let legacy_key = -2_000_000_000i64 + key;
        self.rv_by_kpz.get(&kpz_id)?.get(&legacy_key).copied()
    }

    /// Унифицированное чтение RV с каскадом источников: scoped ARX -> direct key -> address -> legacy fallback; на промахе возвращает `0.0`.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `key`: логический/служебный ключ регистра или индекса.
    /// # Возвращает
    /// - `f64`: значение RV по каскаду резолва; `0.0` при промахе.
    /// # Пример
    /// - `let v = state.get_rv(kpz_id, 71001);`
    #[cfg(test)]
    pub(super) fn get_rv(&self, kpz_id: i32, key: i64) -> f64 {
        let Some(rv) = self.rv_by_kpz.get(&kpz_id) else {
            return 0.0;
        };
        let base = svc_base_for(kpz_id);
        let scoped_min = base + 40000;
        let scoped_max = base + 40000 + 70000;

        // Scoped ARX index keys must not fall back to regular address mapping.
        if key >= scoped_min && key < scoped_max {
            if let Some(v) = rv.get(&key) {
                tracing::debug!(
                    kpz_id = kpz_id,
                    key = key,
                    source = "scoped",
                    value = v,
                    "rv resolved"
                );
                return *v;
            }
            tracing::debug!(
                kpz_id = kpz_id,
                key = key,
                source = "scoped-miss",
                "rv resolved to default"
            );
            return 0.0;
        }

        // ARX index keys (71000 + arx_id) must read only index values,
        // never fallback to regular register-address mapping.
        if (71000..80000).contains(&(key as i32)) {
            let arx_id = (key as i32) - 71000;
            if arx_id > 0 {
                let scoped_key = svc_arx_key(kpz_id, arx_id);
                if let Some(v) = rv.get(&scoped_key) {
                    tracing::debug!(
                        kpz_id = kpz_id,
                        key = key,
                        scoped_key = scoped_key,
                        source = "scoped",
                        value = v,
                        "rv resolved"
                    );
                    return *v;
                }
            }
            tracing::debug!(
                kpz_id = kpz_id,
                key = key,
                source = "scoped-miss",
                "rv resolved to default"
            );
            return 0.0;
        }
        if let Some(v) = rv.get(&key) {
            tracing::debug!(
                kpz_id = kpz_id,
                key = key,
                source = "direct",
                value = v,
                "rv resolved"
            );
            return *v;
        }
        if let Some(v) = self.rv_addr(kpz_id, key as i32) {
            tracing::debug!(
                kpz_id = kpz_id,
                key = key,
                source = "address",
                value = v,
                "rv resolved"
            );
            return v;
        }
        if Self::ENABLE_LEGACY_RV_FALLBACK {
            if let Some(v) = self.rv_legacy(kpz_id, key) {
                tracing::debug!(
                    kpz_id = kpz_id,
                    key = key,
                    source = "legacy",
                    value = v,
                    "rv resolved"
                );
                return v;
            }
        }
        tracing::debug!(
            kpz_id = kpz_id,
            key = key,
            source = "miss",
            "rv resolved to default"
        );
        0.0
    }

    /// Записывает значение в runtime-кэш RV для заданного `kpz_id` и ключа.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `key`: логический/служебный ключ регистра или индекса.
    /// - `val`: числовое значение, которое записывается в runtime-кэш или в связанный регистр.
    /// # Возвращает
    /// - результат согласно типу, указанному в сигнатуре функции.
    /// # Пример
    /// - `state.set_rv(kpz_id, svc_key(kpz_id, 60000), 1.0);`
    pub(super) fn set_rv(&mut self, kpz_id: i32, key: i64, val: f64) {
        let rv = self
            .rv_by_kpz
            .entry(kpz_id)
            .or_insert_with(|| Arc::new(HashMap::new()));
        let rv = Arc::make_mut(rv);
        let changed = match rv.get(&key).copied() {
            Some(prev) => value_changed(Some(prev), val),
            None => true,
        };
        if changed {
            rv.insert(key, val);
            self.rv_dirty = true;
        }
    }

    /// Записывает значение RV по `reg_id`, преобразуя его во внутренний ключ по адресу.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `reg_id`: идентификатор регистра в БД.
    /// - `val`: числовое значение, которое записывается в runtime-кэш или в связанный регистр.
    /// # Возвращает
    /// - результат согласно типу, указанному в сигнатуре функции.
    /// # Пример
    /// - `state.set_rv_reg_value(kpz_id, reg_id, value);`
    pub(super) fn set_rv_reg_value(&mut self, kpz_id: i32, reg_id: i32, val: f64) {
        if let Some(addr) = self.addr_by_reg_id.get(&reg_id).copied() {
            self.set_rv(kpz_id, addr as i64, val);
        }
    }

    /// Синхронизирует из БД состояние ARX индексов (`last_ind`) в scoped RV-ключи и очищает устаревшие ARX-ключи перед загрузкой.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `client`: асинхронный клиент PostgreSQL для чтения/записи runtime-данных.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// # Возвращает
    /// - `Result<()>`: состояние ARX синхронизировано в RV.
    /// # Пример
    /// - `state.prime_arx_state(&client, kpz_id).await?;`
    #[cfg(test)]
    pub(super) async fn prime_arx_state(
        &mut self,
        client: &tokio_postgres::Client,
        kpz_id: i32,
    ) -> Result<()> {
        let base = svc_base_for(kpz_id);
        self.set_rv(kpz_id, 70010, base as f64);
        // Re-sync ARX scoped keys from DB every script cycle to keep memory bounded,
        // but preserve quality keys for currently tracked index addresses.
        let min_scoped = base + 40000;
        let max_scoped = base + 40000 + 70000;
        let mut keep_quality_keys: HashSet<i64> = HashSet::new();
        for (k, _seen) in self.idx_seen.iter() {
            if ((*k >> 32) as i32) != kpz_id {
                continue;
            }
            let addr = (*k & 0xFFFF_FFFF) as u32 as i32;
            if !(0..=65535).contains(&addr) {
                continue;
            }
            keep_quality_keys.insert(svc_key(kpz_id, 60000 + addr * 2));
            keep_quality_keys.insert(svc_key(kpz_id, 60000 + addr * 2 + 1));
            keep_quality_keys.insert(svc_key(kpz_id, 80000 + addr));
        }
        if let Some(rv) = self.rv_by_kpz.get_mut(&kpz_id) {
            let rv = Arc::make_mut(rv);
            let before = rv.len();
            rv.retain(|k, _| {
                keep_quality_keys.contains(k)
                    || !(*k >= min_scoped && *k < max_scoped)
                    || !(*k >= 71000 && *k < 80000)
            });
            if rv.len() != before {
                self.rv_dirty = true;
            }
        }

        let st = load_arx_state_map(client, kpz_id).await?;
        tracing::debug!(
            kpz_id = kpz_id,
            arx_state = st.len(),
            "prime_arx_state loaded"
        );
        for (arx_id, last_ind) in st {
            let scoped_key = svc_arx_key(kpz_id, arx_id);
            let v = last_ind as f64;
            self.set_rv(kpz_id, scoped_key, v);
        }
        Ok(())
    }

    /// Прогревает RV-кэш последними значениями из `arx_val` для КПЗ при старте/ресинке.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `client`: асинхронный клиент PostgreSQL для чтения/записи runtime-данных.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// # Возвращает
    /// - `Result<()>`: кэш RV прогрет из последних `arx_val`.
    /// # Пример
    /// - `state.prime_rv_cache(&client, kpz_id).await?;`
    pub(super) async fn prime_rv_cache(
        &mut self,
        client: &tokio_postgres::Client,
        kpz_id: i32,
    ) -> Result<()> {
        let latest = load_latest_arx_val_map(client, kpz_id).await?;
        for (reg_id, val) in latest.iter().copied() {
            self.set_rv_reg_value(kpz_id, reg_id, val);
        }
        tracing::debug!(
            kpz_id = kpz_id,
            regs = latest.len(),
            "rv cache primed from arx_val"
        );
        Ok(())
    }

    /// Применяет script write-back (`910/911/912`) в таблицу `arx_state` и сразу обновляет соответствующий scoped RV-ключ.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `client`: асинхронный клиент PostgreSQL для чтения/записи runtime-данных.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `out_regs`: карта вычисленных регистров скрипта (ключ -> значение).
    /// # Возвращает
    /// - `Result<()>`: write-back обработан и при необходимости записан в БД/RV.
    /// # Пример
    /// - `state.apply_write_back(&client, kpz_id, &post_out.regs).await?;`
    #[cfg(test)]
    pub(super) async fn apply_write_back(
        &mut self,
        client: &tokio_postgres::Client,
        kpz_id: i32,
        out_regs: &HashMap<i32, f64>,
    ) -> Result<()> {
        let flag = out_regs.get(&910).copied().unwrap_or(0.0) as i32;
        if flag == 0 {
            return Ok(());
        }
        let key = out_regs.get(&911).copied().unwrap_or(0.0) as i32;
        let val = out_regs.get(&912).copied().unwrap_or(0.0) as i32;
        if key <= 0 {
            return Ok(());
        }
        let arx_id = key - 71000;
        if arx_id <= 0 {
            return Ok(());
        }
        set_arx_last_ind(client, kpz_id, arx_id, val).await?;
        tracing::debug!(
            kpz_id = kpz_id,
            key = key,
            arx_id = arx_id,
            val = val,
            "script write_back updated arx_state"
        );
        let v = val as f64;
        self.set_rv(kpz_id, svc_arx_key(kpz_id, arx_id), v);
        Ok(())
    }

    /// Обновляет `arx_state.last_ind`, если post-ключ попадает в scoped диапазон ARX индексов, и отражает новое значение в RV.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `client`: асинхронный клиент PostgreSQL для чтения/записи runtime-данных.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `key`: логический/служебный ключ регистра или индекса.
    /// - `new_ind`: новое значение индекса ARX для записи в состояние.
    /// # Возвращает
    /// - `Result<()>`: индекс ARX обновлен при попадании ключа в scoped-диапазон.
    /// # Пример
    /// - `state.apply_arx_index_update(&client, kpz_id, key, new_ind).await?;`
    #[cfg(test)]
    pub(super) async fn apply_arx_index_update(
        &mut self,
        client: &tokio_postgres::Client,
        kpz_id: i32,
        key: i32,
        new_ind: i32,
    ) -> Result<()> {
        let base = svc_base_for(kpz_id);
        let min_k = base + 40000;
        let max_k = base + 40000 + 70000;
        let key64 = key as i64;
        if key64 >= min_k && key64 < max_k {
            let arx_id = (key64 - (base + 40000)) as i32;
            set_arx_last_ind(client, kpz_id, arx_id, new_ind).await?;
            tracing::debug!(
                kpz_id = kpz_id,
                key = key,
                arx_id = arx_id,
                new_ind = new_ind,
                "arx index update applied"
            );
            let v = new_ind as f64;
            self.set_rv(kpz_id, key64, v);
        }
        Ok(())
    }

    /// Обновляет служебные признаки качества индекса (`ready/stable/quality`) по адресу; требует минимум 2 подтвержденных сэмпла и пишет debug только при изменении значений.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `addr_human`: человекочитаемый адрес регистра Modbus.
    /// - `ts_unix`: текущее время события в Unix-секундах.
    /// - `value`: новое числовое значение регистра для проверки/обновления.
    /// # Возвращает
    /// - результат согласно типу, указанному в сигнатуре функции.
    /// # Пример
    /// - `state.update_idx_quality(kpz_id, 400, now_unix(), value);`
    #[cfg(test)]
    pub(super) fn update_idx_quality(
        &mut self,
        kpz_id: i32,
        addr_human: i32,
        ts_unix: i64,
        value: f64,
    ) {
        if !(0..=65535).contains(&addr_human) {
            return;
        }

        let key = ((kpz_id as i64) << 32) | (addr_human as u32 as i64);
        let samples = {
            let seen = self.idx_seen.entry(key).or_default();
            let by_kpz_seen = self.idx_seen_by_kpz.entry(kpz_id).or_default();
            if seen.last_ts != 0 && ts_unix <= seen.last_ts {
                return;
            }
            seen.last_ts = ts_unix;
            if seen.samples < 2 {
                seen.samples += 1;
            }
            by_kpz_seen.insert(addr_human, *seen);
            seen.samples
        };

        let slot = addr_human;
        let ready_key = svc_key(kpz_id, 60000 + slot * 2);
        let stable_key = svc_key(kpz_id, 60000 + slot * 2 + 1);
        let quality_key = svc_key(kpz_id, 80000 + slot);
        let (prev_ready, prev_stable, prev_quality) = self
            .rv_by_kpz
            .get(&kpz_id)
            .map(|rv| {
                (
                    rv.get(&ready_key).copied(),
                    rv.get(&stable_key).copied(),
                    rv.get(&quality_key).copied(),
                )
            })
            .unwrap_or((None, None, None));

        if samples >= 2 {
            self.set_rv(kpz_id, ready_key, 1.0);
            self.set_rv(kpz_id, stable_key, value);
            self.set_rv(kpz_id, quality_key, 100.0);
            if value_changed(prev_ready, 1.0)
                || value_changed(prev_stable, value)
                || value_changed(prev_quality, 100.0)
            {
                tracing::debug!(
                    kpz_id = kpz_id,
                    addr = addr_human,
                    ts = ts_unix,
                    samples = samples,
                    value = value,
                    quality = 100.0,
                    "idx quality updated"
                );
            }
        } else {
            self.set_rv(kpz_id, ready_key, 0.0);
            self.set_rv(kpz_id, quality_key, 0.0);
            if value_changed(prev_ready, 0.0) || value_changed(prev_quality, 0.0) {
                tracing::debug!(
                    kpz_id = kpz_id,
                    addr = addr_human,
                    ts = ts_unix,
                    samples = samples,
                    value = value,
                    quality = 0.0,
                    "idx quality updated"
                );
            }
        }
    }

    /// Сбрасывает качество индекса в `0` для устаревших адресов (по таймауту stale) и требует повторное подтверждение свежими сэмплами.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `kpz_id`: идентификатор КПЗ, для которого выполняется операция.
    /// - `now_ts`: текущее Unix-время для проверки устаревания качества.
    /// # Возвращает
    /// - результат согласно типу, указанному в сигнатуре функции.
    /// # Пример
    /// - `state.refresh_idx_quality_staleness(kpz_id, now_unix());`
    #[cfg(test)]
    pub(super) fn refresh_idx_quality_staleness(&mut self, kpz_id: i32, now_ts: i64) {
        let mut stale_addrs: Vec<i32> = Vec::new();
        for (k, seen) in self.idx_seen.iter_mut() {
            if ((*k >> 32) as i32) != kpz_id {
                continue;
            }
            if seen.last_ts <= 0 {
                continue;
            }
            if now_ts - seen.last_ts <= IDX_QUALITY_STALE_SEC {
                continue;
            }
            let addr = (*k & 0xFFFF_FFFF) as u32 as i32;
            stale_addrs.push(addr);
            // Require two fresh confirmations after stale timeout.
            seen.samples = 0;
            if let Some(by_kpz_seen) = self.idx_seen_by_kpz.get_mut(&kpz_id) {
                by_kpz_seen.insert(addr, *seen);
            }
        }

        for addr in stale_addrs {
            let slot = addr;
            let ready_key = svc_key(kpz_id, 60000 + slot * 2);
            let quality_key = svc_key(kpz_id, 80000 + slot);
            self.set_rv(kpz_id, ready_key, 0.0);
            self.set_rv(kpz_id, quality_key, 0.0);
            tracing::debug!(
                kpz_id = kpz_id,
                addr = addr,
                now_ts = now_ts,
                stale_after_sec = IDX_QUALITY_STALE_SEC,
                "idx quality reset due to stale timeout"
            );
        }
    }

    /// Резолвит входной ключ скрипта в `reg_id`: сначала как прямой `reg_id`, затем как `addr -> reg_id`.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `key`: логический/служебный ключ регистра или индекса.
    /// # Возвращает
    /// - результат согласно типу, указанному в сигнатуре функции.
    /// # Пример
    /// - `let reg_id = state.map_reg_id(key);`
    #[cfg(test)]
    pub(super) fn map_reg_id(&self, key: i32) -> Option<i32> {
        if self.tip_by_reg_id.contains_key(&key) {
            return Some(key);
        }
        self.reg_id_by_addr.get(&key).copied()
    }

    /// Возвращает тип регистра (`tip`) по `reg_id`; если не найден, использует безопасный fallback `5`.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `reg_id`: идентификатор регистра в БД.
    /// # Возвращает
    /// - результат согласно типу, указанному в сигнатуре функции.
    /// # Пример
    /// - `let tip = state.tip_of(reg_id);`
    #[cfg(test)]
    pub(super) fn tip_of(&self, reg_id: i32) -> i32 {
        *self.tip_by_reg_id.get(&reg_id).unwrap_or(&5)
    }
}

impl WorkerCtx {
    const ENABLE_LEGACY_RV_FALLBACK: bool = true;

    pub(super) fn rv_reg_id(&self, kpz_id: i32, reg_id: i32) -> Option<f64> {
        if kpz_id != self.kpz_id {
            return None;
        }
        let addr = *self.shared.addr_by_reg_id.get(&reg_id)?;
        self.runtime.rv.get(&(addr as i64)).copied()
    }

    pub(super) fn rv_addr(&self, kpz_id: i32, addr_human: i32) -> Option<f64> {
        let reg_id = *self.shared.reg_id_by_addr.get(&addr_human)?;
        self.rv_reg_id(kpz_id, reg_id)
    }

    pub(super) fn rv_svc(&self, kpz_id: i32, offset: i32) -> Option<f64> {
        if kpz_id != self.kpz_id {
            return None;
        }
        let key = svc_key(kpz_id, offset);
        self.runtime.rv.get(&key).copied()
    }

    pub(super) fn rv_legacy(&self, kpz_id: i32, key: i64) -> Option<f64> {
        if kpz_id != self.kpz_id {
            return None;
        }
        let legacy_key = -2_000_000_000i64 + key;
        self.runtime.rv.get(&legacy_key).copied()
    }

    pub(super) fn get_rv(&self, kpz_id: i32, key: i64) -> f64 {
        if kpz_id != self.kpz_id {
            return 0.0;
        }
        let rv = self.runtime.rv.as_ref();
        let base = svc_base_for(kpz_id);
        let scoped_min = base + 40000;
        let scoped_max = base + 40000 + 70000;

        if key >= scoped_min && key < scoped_max {
            if let Some(v) = rv.get(&key) {
                tracing::debug!(
                    kpz_id = kpz_id,
                    key = key,
                    source = "scoped",
                    value = v,
                    "rv resolved"
                );
                return *v;
            }
            tracing::debug!(
                kpz_id = kpz_id,
                key = key,
                source = "scoped-miss",
                "rv resolved to default"
            );
            return 0.0;
        }

        if (71000..80000).contains(&(key as i32)) {
            let arx_id = (key as i32) - 71000;
            if arx_id > 0 {
                let scoped_key = svc_arx_key(kpz_id, arx_id);
                if let Some(v) = rv.get(&scoped_key) {
                    tracing::debug!(
                        kpz_id = kpz_id,
                        key = key,
                        scoped_key = scoped_key,
                        source = "scoped",
                        value = v,
                        "rv resolved"
                    );
                    return *v;
                }
            }
            tracing::debug!(
                kpz_id = kpz_id,
                key = key,
                source = "scoped-miss",
                "rv resolved to default"
            );
            return 0.0;
        }
        if let Some(v) = rv.get(&key) {
            tracing::debug!(
                kpz_id = kpz_id,
                key = key,
                source = "direct",
                value = v,
                "rv resolved"
            );
            return *v;
        }
        if let Some(v) = self.rv_addr(kpz_id, key as i32) {
            tracing::debug!(
                kpz_id = kpz_id,
                key = key,
                source = "address",
                value = v,
                "rv resolved"
            );
            return v;
        }
        if Self::ENABLE_LEGACY_RV_FALLBACK {
            if let Some(v) = self.rv_legacy(kpz_id, key) {
                tracing::debug!(
                    kpz_id = kpz_id,
                    key = key,
                    source = "legacy",
                    value = v,
                    "rv resolved"
                );
                return v;
            }
        }
        tracing::debug!(
            kpz_id = kpz_id,
            key = key,
            source = "miss",
            "rv resolved to default"
        );
        0.0
    }

    pub(super) fn set_rv(&mut self, kpz_id: i32, key: i64, val: f64) {
        if kpz_id == self.kpz_id {
            let rv = Arc::make_mut(&mut self.runtime.rv);
            rv.insert(key, val);
            self.runtime.rv_dirty = true;
        }
    }

    pub(super) fn set_rv_reg_value(&mut self, kpz_id: i32, reg_id: i32, val: f64) {
        if let Some(addr) = self.shared.addr_by_reg_id.get(&reg_id).copied() {
            self.set_rv(kpz_id, addr as i64, val);
        }
    }

    pub(super) async fn prime_arx_state(
        &mut self,
        client: &tokio_postgres::Client,
        kpz_id: i32,
    ) -> Result<()> {
        let base = svc_base_for(kpz_id);
        self.set_rv(kpz_id, 70010, base as f64);
        let min_scoped = base + 40000;
        let max_scoped = base + 40000 + 70000;
        let mut keep_quality_keys: HashSet<i64> = HashSet::new();
        for addr in self.runtime.idx_seen.keys().copied() {
            if !(0..=65535).contains(&addr) {
                continue;
            }
            keep_quality_keys.insert(svc_key(kpz_id, 60000 + addr * 2));
            keep_quality_keys.insert(svc_key(kpz_id, 60000 + addr * 2 + 1));
            keep_quality_keys.insert(svc_key(kpz_id, 80000 + addr));
        }
        if kpz_id == self.kpz_id {
            let rv = Arc::make_mut(&mut self.runtime.rv);
            rv.retain(|k, _| {
                keep_quality_keys.contains(k)
                    || !(*k >= min_scoped && *k < max_scoped)
                    || !(*k >= 71000 && *k < 80000)
            });
        }

        let st = load_arx_state_map(client, kpz_id).await?;
        tracing::debug!(
            kpz_id = kpz_id,
            arx_state = st.len(),
            "prime_arx_state loaded"
        );
        for (arx_id, last_ind) in st {
            self.set_rv(kpz_id, svc_arx_key(kpz_id, arx_id), last_ind as f64);
        }
        Ok(())
    }

    pub(super) async fn apply_write_back(
        &mut self,
        _client: &tokio_postgres::Client,
        kpz_id: i32,
        out_regs: &HashMap<i32, f64>,
    ) -> Result<()> {
        let flag = out_regs.get(&910).copied().unwrap_or(0.0) as i32;
        if flag == 0 {
            return Ok(());
        }
        let key = out_regs.get(&911).copied().unwrap_or(0.0) as i32;
        let val = out_regs.get(&912).copied().unwrap_or(0.0) as i32;
        if key <= 0 {
            return Ok(());
        }
        let arx_id = key - 71000;
        if arx_id <= 0 {
            return Ok(());
        }
        self.db_delta.arx_state_updates.push(ArxStateUpdate {
            kpz_id,
            arx_id,
            last_ind: val,
        });
        tracing::debug!(
            kpz_id = kpz_id,
            key = key,
            arx_id = arx_id,
            val = val,
            "script write_back updated arx_state"
        );
        self.set_rv(kpz_id, svc_arx_key(kpz_id, arx_id), val as f64);
        Ok(())
    }

    pub(super) async fn apply_arx_index_update(
        &mut self,
        _client: &tokio_postgres::Client,
        kpz_id: i32,
        key: i32,
        new_ind: i32,
    ) -> Result<()> {
        let base = svc_base_for(kpz_id);
        let min_k = base + 40000;
        let max_k = base + 40000 + 70000;
        let key64 = key as i64;
        if key64 >= min_k && key64 < max_k {
            let arx_id = (key64 - (base + 40000)) as i32;
            self.db_delta.arx_state_updates.push(ArxStateUpdate {
                kpz_id,
                arx_id,
                last_ind: new_ind,
            });
            tracing::debug!(
                kpz_id = kpz_id,
                key = key,
                arx_id = arx_id,
                new_ind = new_ind,
                "arx index update applied"
            );
            self.set_rv(kpz_id, key64, new_ind as f64);
        }
        Ok(())
    }

    pub(super) fn update_idx_quality(
        &mut self,
        kpz_id: i32,
        addr_human: i32,
        ts_unix: i64,
        value: f64,
    ) {
        if !(0..=65535).contains(&addr_human) {
            return;
        }
        let samples = {
            let idx_seen = Arc::make_mut(&mut self.runtime.idx_seen);
            let seen = idx_seen.entry(addr_human).or_default();
            self.runtime.idx_seen_dirty = true;
            if seen.last_ts != 0 && ts_unix <= seen.last_ts {
                return;
            }
            seen.last_ts = ts_unix;
            if seen.samples < 2 {
                seen.samples += 1;
            }
            seen.samples
        };

        let slot = addr_human;
        let ready_key = svc_key(kpz_id, 60000 + slot * 2);
        let stable_key = svc_key(kpz_id, 60000 + slot * 2 + 1);
        let quality_key = svc_key(kpz_id, 80000 + slot);

        if samples >= 2 {
            self.set_rv(kpz_id, ready_key, 1.0);
            self.set_rv(kpz_id, stable_key, value);
            self.set_rv(kpz_id, quality_key, 100.0);
        } else {
            self.set_rv(kpz_id, ready_key, 0.0);
            self.set_rv(kpz_id, quality_key, 0.0);
        }
    }

    pub(super) fn refresh_idx_quality_staleness(&mut self, kpz_id: i32, now_ts: i64) {
        let mut stale_addrs: Vec<i32> = Vec::new();
        let idx_seen = Arc::make_mut(&mut self.runtime.idx_seen);
        self.runtime.idx_seen_dirty = true;
        for (addr, seen) in idx_seen.iter_mut() {
            if seen.last_ts <= 0 {
                continue;
            }
            if now_ts - seen.last_ts <= IDX_QUALITY_STALE_SEC {
                continue;
            }
            stale_addrs.push(*addr);
            seen.samples = 0;
        }

        for addr in stale_addrs {
            let slot = addr;
            self.set_rv(kpz_id, svc_key(kpz_id, 60000 + slot * 2), 0.0);
            self.set_rv(kpz_id, svc_key(kpz_id, 80000 + slot), 0.0);
        }
    }

    pub(super) fn map_reg_id(&self, key: i32) -> Option<i32> {
        if self.shared.tip_by_reg_id.contains_key(&key) {
            return Some(key);
        }
        self.shared.reg_id_by_addr.get(&key).copied()
    }

    pub(super) fn tip_of(&self, reg_id: i32) -> i32 {
        *self.shared.tip_by_reg_id.get(&reg_id).unwrap_or(&5)
    }
}
