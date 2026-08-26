//! Types and logic for batched DB writes: `DbDelta` and row types used by merge/db_writer/worker.

use std::collections::HashMap;

use crate::db_queries::ElamRow;
use crate::types::ArxValRow;

#[derive(Clone, Default)]
pub(super) struct DbDelta {
    pub arx_rows: Vec<ArxValRow>,
    pub elam_rows: Vec<ElamRow>,
    pub poll_logs: Vec<PollLogRow>,
    pub alarm_state_updates: Vec<AlarmStateUpdate>,
    pub alarm_events: Vec<AlarmEventRow>,
    pub arx_state_updates: Vec<ArxStateUpdate>,
}

impl DbDelta {
    pub(super) fn is_empty(&self) -> bool {
        self.arx_rows.is_empty()
            && self.elam_rows.is_empty()
            && self.poll_logs.is_empty()
            && self.alarm_state_updates.is_empty()
            && self.alarm_events.is_empty()
            && self.arx_state_updates.is_empty()
    }

    pub(super) fn append_coalescing_runtime_updates(
        &mut self,
        mut other: DbDelta,
    ) -> (usize, usize) {
        self.arx_rows.append(&mut other.arx_rows);
        self.elam_rows.append(&mut other.elam_rows);
        self.poll_logs.append(&mut other.poll_logs);
        self.alarm_events.append(&mut other.alarm_events);

        let mut coalesced_alarm = 0usize;
        let mut alarm_idx_by_rule: HashMap<i64, usize> = self
            .alarm_state_updates
            .iter()
            .enumerate()
            .map(|(idx, update)| (update.rule_id, idx))
            .collect();
        for update in other.alarm_state_updates.drain(..) {
            if let Some(existing_idx) = alarm_idx_by_rule.get(&update.rule_id).copied() {
                self.alarm_state_updates[existing_idx] = update;
                coalesced_alarm += 1;
            } else {
                let idx = self.alarm_state_updates.len();
                alarm_idx_by_rule.insert(update.rule_id, idx);
                self.alarm_state_updates.push(update);
            }
        }

        let mut coalesced_arx = 0usize;
        let mut arx_idx_by_key: HashMap<(i32, i32), usize> = self
            .arx_state_updates
            .iter()
            .enumerate()
            .map(|(idx, update)| ((update.kpz_id, update.arx_id), idx))
            .collect();
        for update in other.arx_state_updates.drain(..) {
            let key = (update.kpz_id, update.arx_id);
            if let Some(existing_idx) = arx_idx_by_key.get(&key).copied() {
                self.arx_state_updates[existing_idx] = update;
                coalesced_arx += 1;
            } else {
                let idx = self.arx_state_updates.len();
                arx_idx_by_key.insert(key, idx);
                self.arx_state_updates.push(update);
            }
        }

        (coalesced_alarm, coalesced_arx)
    }

    pub(super) fn total_rows(&self) -> usize {
        self.arx_rows.len()
            + self.elam_rows.len()
            + self.poll_logs.len()
            + self.alarm_state_updates.len()
            + self.alarm_events.len()
            + self.arx_state_updates.len()
    }

    pub(super) fn drop_poll_logs(&mut self) -> usize {
        let dropped = self.poll_logs.len();
        self.poll_logs.clear();
        dropped
    }

    #[cfg(test)]
    pub(super) fn coalesce_runtime_updates(&mut self) -> (usize, usize) {
        let alarm_before = self.alarm_state_updates.len();
        if alarm_before > 1 {
            let mut last_by_rule: HashMap<i64, AlarmStateUpdate> =
                HashMap::with_capacity(alarm_before);
            for update in self.alarm_state_updates.drain(..) {
                last_by_rule.insert(update.rule_id, update);
            }
            self.alarm_state_updates = last_by_rule.into_values().collect();
        }

        let arx_before = self.arx_state_updates.len();
        if arx_before > 1 {
            let mut last_by_key: HashMap<(i32, i32), ArxStateUpdate> =
                HashMap::with_capacity(arx_before);
            for update in self.arx_state_updates.drain(..) {
                last_by_key.insert((update.kpz_id, update.arx_id), update);
            }
            self.arx_state_updates = last_by_key.into_values().collect();
        }

        (
            alarm_before.saturating_sub(self.alarm_state_updates.len()),
            arx_before.saturating_sub(self.arx_state_updates.len()),
        )
    }
}

#[derive(Clone)]
pub(super) struct PollLogRow {
    pub kpz_id: Option<i32>,
    pub kind: String,
    pub msg: String,
}

#[derive(Clone)]
pub(super) struct AlarmStateUpdate {
    pub rule_id: i64,
    pub active: bool,
    pub value: f64,
}

#[derive(Clone)]
pub(super) struct AlarmEventRow {
    pub kpz_id: i32,
    pub reg_id: i32,
    pub rule_id: i64,
    pub event: &'static str,
    pub value: f64,
    pub set_lo: Option<f64>,
    pub set_hi: Option<f64>,
    pub severity: i16,
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone)]
pub(super) struct ArxStateUpdate {
    pub kpz_id: i32,
    pub arx_id: i32,
    pub last_ind: i32,
}
