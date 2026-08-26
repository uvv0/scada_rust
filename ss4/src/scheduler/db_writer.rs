use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::merge;
use super::DbDelta;

pub(super) struct DbWriter {
    tx: mpsc::Sender<DbDelta>,
    join: JoinHandle<Result<()>>,
    stats: Arc<DbWriterStats>,
}

#[derive(Default)]
struct DbWriterStats {
    enqueued_batches: AtomicU64,
    enqueued_rows: AtomicU64,
    flushed_batches: AtomicU64,
    flushed_rows: AtomicU64,
    flush_errors: AtomicU64,
    dropped_poll_logs: AtomicU64,
    shed_batches: AtomicU64,
    coalesced_alarm_state_updates: AtomicU64,
    coalesced_arx_state_updates: AtomicU64,
    max_queue_depth: AtomicUsize,
    last_flush_rows: AtomicU64,
    last_flush_ms: AtomicU64,
}

impl DbWriter {
    pub(super) fn start(client: Arc<tokio_postgres::Client>, capacity: usize) -> Self {
        let stats = Arc::new(DbWriterStats::default());
        let (tx, mut rx) = mpsc::channel::<DbDelta>(capacity.max(1));
        let stats_cl = Arc::clone(&stats);
        let join = tokio::spawn(async move {
            while let Some(first) = rx.recv().await {
                let mut batch = DbDelta::default();
                let (mut coalesced_alarm, mut coalesced_arx) =
                    batch.append_coalescing_runtime_updates(first);
                while let Ok(next) = rx.try_recv() {
                    let (a, x) = batch.append_coalescing_runtime_updates(next);
                    coalesced_alarm += a;
                    coalesced_arx += x;
                }
                if coalesced_alarm > 0 {
                    stats_cl
                        .coalesced_alarm_state_updates
                        .fetch_add(coalesced_alarm as u64, Ordering::Relaxed);
                }
                if coalesced_arx > 0 {
                    stats_cl
                        .coalesced_arx_state_updates
                        .fetch_add(coalesced_arx as u64, Ordering::Relaxed);
                }
                let started = Instant::now();
                let rows = batch.total_rows() as u64;
                let flush_res = merge::flush_db_delta(client.as_ref(), &batch).await;
                stats_cl.last_flush_rows.store(rows, Ordering::Relaxed);
                stats_cl
                    .last_flush_ms
                    .store(started.elapsed().as_millis() as u64, Ordering::Relaxed);
                match flush_res {
                    Ok(()) => {
                        stats_cl.flushed_batches.fetch_add(1, Ordering::Relaxed);
                        stats_cl.flushed_rows.fetch_add(rows, Ordering::Relaxed);
                    }
                    Err(err) => {
                        stats_cl.flush_errors.fetch_add(1, Ordering::Relaxed);
                        return Err(err);
                    }
                }
            }
            Ok(())
        });
        Self { tx, join, stats }
    }

    pub(super) async fn enqueue(&self, db_delta: DbDelta) -> Result<()> {
        let queue_depth = self.tx.max_capacity() - self.tx.capacity();
        self.stats
            .max_queue_depth
            .fetch_max(queue_depth, Ordering::Relaxed);

        let mut db_delta = db_delta;
        if queue_depth >= self.backpressure_shed_threshold() {
            let dropped_poll_logs = db_delta.drop_poll_logs() as u64;
            if dropped_poll_logs > 0 {
                self.stats
                    .dropped_poll_logs
                    .fetch_add(dropped_poll_logs, Ordering::Relaxed);
                self.stats.shed_batches.fetch_add(1, Ordering::Relaxed);
            }
        }

        if db_delta.is_empty() {
            return Ok(());
        }
        let rows = db_delta.total_rows() as u64;
        self.stats.enqueued_batches.fetch_add(1, Ordering::Relaxed);
        self.stats.enqueued_rows.fetch_add(rows, Ordering::Relaxed);
        self.tx
            .send(db_delta)
            .await
            .map_err(|e| anyhow::anyhow!("db writer queue closed: {}", e))
    }

    fn backpressure_shed_threshold(&self) -> usize {
        let cap = self.tx.max_capacity();
        if cap <= 1 {
            return 1;
        }
        ((cap * 3) / 4).max(1)
    }

    pub(super) fn log_stats(&self) {
        tracing::info!(
            enqueued_batches = self.stats.enqueued_batches.load(Ordering::Relaxed),
            enqueued_rows = self.stats.enqueued_rows.load(Ordering::Relaxed),
            flushed_batches = self.stats.flushed_batches.load(Ordering::Relaxed),
            flushed_rows = self.stats.flushed_rows.load(Ordering::Relaxed),
            flush_errors = self.stats.flush_errors.load(Ordering::Relaxed),
            dropped_poll_logs = self.stats.dropped_poll_logs.load(Ordering::Relaxed),
            shed_batches = self.stats.shed_batches.load(Ordering::Relaxed),
            coalesced_alarm_state_updates = self
                .stats
                .coalesced_alarm_state_updates
                .load(Ordering::Relaxed),
            coalesced_arx_state_updates = self
                .stats
                .coalesced_arx_state_updates
                .load(Ordering::Relaxed),
            max_queue_depth = self.stats.max_queue_depth.load(Ordering::Relaxed),
            last_flush_rows = self.stats.last_flush_rows.load(Ordering::Relaxed),
            last_flush_ms = self.stats.last_flush_ms.load(Ordering::Relaxed),
            queue_depth = (self.tx.max_capacity() - self.tx.capacity()),
            queue_capacity = self.tx.max_capacity(),
            "db writer metrics"
        );
    }

    pub(super) async fn finish(self) -> Result<()> {
        drop(self.tx);
        let join_res = self
            .join
            .await
            .map_err(|e| anyhow::anyhow!("db writer task join error: {}", e))?;
        tracing::info!(
            enqueued_batches = self.stats.enqueued_batches.load(Ordering::Relaxed),
            enqueued_rows = self.stats.enqueued_rows.load(Ordering::Relaxed),
            flushed_batches = self.stats.flushed_batches.load(Ordering::Relaxed),
            flushed_rows = self.stats.flushed_rows.load(Ordering::Relaxed),
            flush_errors = self.stats.flush_errors.load(Ordering::Relaxed),
            dropped_poll_logs = self.stats.dropped_poll_logs.load(Ordering::Relaxed),
            shed_batches = self.stats.shed_batches.load(Ordering::Relaxed),
            coalesced_alarm_state_updates = self
                .stats
                .coalesced_alarm_state_updates
                .load(Ordering::Relaxed),
            coalesced_arx_state_updates = self
                .stats
                .coalesced_arx_state_updates
                .load(Ordering::Relaxed),
            max_queue_depth = self.stats.max_queue_depth.load(Ordering::Relaxed),
            last_flush_rows = self.stats.last_flush_rows.load(Ordering::Relaxed),
            last_flush_ms = self.stats.last_flush_ms.load(Ordering::Relaxed),
            "db writer stopped"
        );
        join_res
    }
}
