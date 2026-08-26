use super::*;

/// Определяет, относится ли ошибка к таймауту по текстовым признакам.
/// # Параметры
/// - `e`: ошибка для классификации/логирования.
/// # Возвращает
/// - `bool`: `true`, если ошибка классифицирована как timeout.
/// # Пример
/// - `let timed_out = is_timeout_error(&err);`
pub(super) fn is_timeout_error(e: &anyhow::Error) -> bool {
    let s = e.to_string().to_ascii_lowercase();
    s.contains("timeout") || s.contains("timed out")
}

/// Оценивает p-квантиль латентности по гистограмме счетчиков (`<=100/<=300/<=1000/>1000` мс).
/// # Параметры
/// - `total`: общее число измерений в гистограмме.
/// - `le_100`: число измерений с латентностью <=100 мс.
/// - `le_300`: число измерений с латентностью <=300 мс.
/// - `le_1000`: число измерений с латентностью <=1000 мс.
/// - `gt_1000`: число измерений с латентностью >1000 мс.
/// - `p`: целевой квантиль (например, 0.50 или 0.95).
/// # Возвращает
/// - `u64`: оценка квантиля латентности в миллисекундах.
/// # Пример
/// - `let p95 = approx_percentile_ms(total, le100, le300, le1000, gt1000, 0.95);`
fn approx_percentile_ms(
    total: u64,
    le_100: u64,
    le_300: u64,
    le_1000: u64,
    gt_1000: u64,
    p: f64,
) -> u64 {
    if total == 0 {
        return 0;
    }
    let mut target = (total as f64 * p).ceil() as u64;
    if target == 0 {
        target = 1;
    }
    if target <= le_100 {
        return 100;
    }
    if target <= le_100 + le_300 {
        return 300;
    }
    if target <= le_100 + le_300 + le_1000 {
        return 1000;
    }
    if gt_1000 > 0 {
        return 3000;
    }
    1000
}

impl SchedulerState {
    /// Раз в окно метрик вычисляет p50/p95, классифицирует состояние scheduler health (ok/warn/crit), логирует сводку и сбрасывает счетчики.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// # Возвращает
    /// - `Option<(String, String)>`: `(kind, msg)` при наступлении окна метрик, иначе `None`.
    /// # Пример
    /// - `if let Some((kind,msg)) = state.log_metrics_if_due() { /* log */ }`
    pub(super) fn log_metrics_if_due(&mut self) -> Option<(String, String)> {
        let now = Instant::now();
        if now < self.next_metrics_log {
            return None;
        }
        self.next_metrics_log = now + Duration::from_secs(METRICS_EVERY_SEC);

        let started = self.metrics_jobs_started;
        let ok = self.metrics_jobs_ok;
        let err = self.metrics_jobs_err;
        let timeout = self.metrics_jobs_timeout;
        let p50_ms = approx_percentile_ms(
            started,
            self.metrics_lat_le_100_ms,
            self.metrics_lat_le_300_ms,
            self.metrics_lat_le_1000_ms,
            self.metrics_lat_gt_1000_ms,
            0.50,
        );
        let p95_ms = approx_percentile_ms(
            started,
            self.metrics_lat_le_100_ms,
            self.metrics_lat_le_300_ms,
            self.metrics_lat_le_1000_ms,
            self.metrics_lat_gt_1000_ms,
            0.95,
        );
        let err_rate = if started > 0 {
            (err as f64) / (started as f64)
        } else {
            0.0
        };
        let timeout_rate = if started > 0 {
            (timeout as f64) / (started as f64)
        } else {
            0.0
        };
        tracing::debug!(
            tasks = self.tasks.len(),
            queue = self.queue.len(),
            rv_kpz = self.rv_by_kpz.len(),
            jobs_started = started,
            jobs_ok = ok,
            jobs_err = err,
            jobs_timeout = timeout,
            lat_p50_ms = p50_ms,
            lat_p95_ms = p95_ms,
            "scheduler metrics"
        );

        if started > 0 && err > 0 {
            self.metrics_err_windows_streak = self.metrics_err_windows_streak.saturating_add(1);
        } else {
            self.metrics_err_windows_streak = 0;
        }
        let critical = err_rate >= METRICS_ERR_RATE_CRIT
            || self.metrics_err_windows_streak >= METRICS_ERR_STREAK_CRIT_WINDOWS
            || p95_ms >= self.metrics_p95_crit_ms;
        let warn = err_rate >= METRICS_ERR_RATE_WARN || p95_ms >= self.metrics_p95_warn_ms;
        let kind = if critical {
            tracing::error!(
                jobs_started = started,
                jobs_ok = ok,
                jobs_err = err,
                jobs_timeout = timeout,
                err_rate = err_rate,
                timeout_rate = timeout_rate,
                lat_p50_ms = p50_ms,
                lat_p95_ms = p95_ms,
                err_windows_streak = self.metrics_err_windows_streak,
                "scheduler health critical"
            );
            "health_crit"
        } else if warn {
            tracing::warn!(
                jobs_started = started,
                jobs_ok = ok,
                jobs_err = err,
                jobs_timeout = timeout,
                err_rate = err_rate,
                timeout_rate = timeout_rate,
                lat_p50_ms = p50_ms,
                lat_p95_ms = p95_ms,
                err_windows_streak = self.metrics_err_windows_streak,
                "scheduler health warning"
            );
            "health_warn"
        } else {
            tracing::info!(
                jobs_started = started,
                jobs_ok = ok,
                jobs_err = err,
                jobs_timeout = timeout,
                err_rate = err_rate,
                timeout_rate = timeout_rate,
                lat_p50_ms = p50_ms,
                lat_p95_ms = p95_ms,
                "scheduler health ok"
            );
            "health_ok"
        };
        let msg = format!(
            "scheduler health: started={}, ok={}, err={}, timeout={}, err_rate={:.2}, timeout_rate={:.2}, p50_ms={}, p95_ms={}, err_windows_streak={}",
            started, ok, err, timeout, err_rate, timeout_rate, p50_ms, p95_ms, self.metrics_err_windows_streak
        );

        self.metrics_jobs_started = 0;
        self.metrics_jobs_ok = 0;
        self.metrics_jobs_err = 0;
        self.metrics_jobs_timeout = 0;
        self.metrics_lat_le_100_ms = 0;
        self.metrics_lat_le_300_ms = 0;
        self.metrics_lat_le_1000_ms = 0;
        self.metrics_lat_gt_1000_ms = 0;
        Some((kind.to_string(), msg))
    }

    /// Добавляет длительность задания в bucket-гистограмму латентности для последующего расчета p50/p95.
    /// # Параметры
    /// - `self`: контекст текущего состояния/экземпляра планировщика.
    /// - `elapsed`: измеренная длительность выполнения задания.
    /// # Возвращает
    /// - `()`: обновляет bucket-счетчики латентности.
    /// # Пример
    /// - `state.record_job_latency(started.elapsed());`
    #[allow(dead_code)]
    pub(super) fn record_job_latency(&mut self, elapsed: Duration) {
        let ms = elapsed.as_millis() as u64;
        if ms <= 100 {
            self.metrics_lat_le_100_ms += 1;
        } else if ms <= 300 {
            self.metrics_lat_le_300_ms += 1;
        } else if ms <= 1000 {
            self.metrics_lat_le_1000_ms += 1;
        } else {
            self.metrics_lat_gt_1000_ms += 1;
        }
    }
}
