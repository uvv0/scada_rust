use anyhow::Result;

use crate::db::Db;
use crate::models::{AlarmEventRow, AlarmRuleRow, AlarmStateRow};

impl Db {
    pub fn get_alarm_rules(&self, kpz_id: Option<i32>) -> Result<Vec<AlarmRuleRow>> {
        self.rt.block_on(async {
            let rows = if let Some(k) = kpz_id {
                self.client
                    .query(
                        "select id, kpz_id, reg_id, enabled, cmp, set_lo, set_hi, set_lo_1, set_hi_1, \
                                coalesce(hysteresis,0), coalesce(on_delay_sec,0), coalesce(off_delay_sec,0), \
                                coalesce(severity,1), code, message \
                         from alarm_rule where kpz_id = $1 order by kpz_id, reg_id, id",
                        &[&k],
                    )
                    .await?
            } else {
                self.client
                    .query(
                        "select id, kpz_id, reg_id, enabled, cmp, set_lo, set_hi, set_lo_1, set_hi_1, \
                                coalesce(hysteresis,0), coalesce(on_delay_sec,0), coalesce(off_delay_sec,0), \
                                coalesce(severity,1), code, message \
                         from alarm_rule order by kpz_id, reg_id, id",
                        &[],
                    )
                    .await?
            };
            let out = rows
                .into_iter()
                .map(|r| AlarmRuleRow {
                    id: r.get::<_, i64>(0),
                    kpz_id: r.get::<_, i32>(1),
                    reg_id: r.get::<_, i32>(2),
                    enabled: r
                        .try_get::<_, bool>(3)
                        .ok()
                        .or_else(|| r.try_get::<_, i16>(3).ok().map(|v| v != 0))
                        .or_else(|| r.try_get::<_, i32>(3).ok().map(|v| v != 0))
                        .unwrap_or(true),
                    cmp: r.get::<_, String>(4),
                    set_lo: r.try_get::<_, Option<f64>>(5).ok().flatten(),
                    set_hi: r.try_get::<_, Option<f64>>(6).ok().flatten(),
                    set_lo_1: r.try_get::<_, Option<f64>>(7).ok().flatten(),
                    set_hi_1: r.try_get::<_, Option<f64>>(8).ok().flatten(),
                    hysteresis: r.get::<_, f64>(9),
                    on_delay_sec: r.get::<_, i32>(10),
                    off_delay_sec: r.get::<_, i32>(11),
                    severity: r
                        .try_get::<_, i16>(12)
                        .ok()
                        .or_else(|| r.try_get::<_, i32>(12).ok().map(|v| v as i16))
                        .unwrap_or(1),
                    code: r.try_get::<_, Option<String>>(13).ok().flatten(),
                    message: r.try_get::<_, Option<String>>(14).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }

    #[allow(dead_code)]
    pub fn upsert_alarm_rule(&self, row: &AlarmRuleRow) -> Result<i64> {
        self.rt.block_on(async {
            let r = self
                .client
                .query_one(
                    "insert into alarm_rule(id, kpz_id, reg_id, enabled, cmp, set_lo, set_hi, set_lo_1, set_hi_1, hysteresis, on_delay_sec, off_delay_sec, severity, code, message) \
                     values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) \
                     on conflict (id) do update set \
                       kpz_id=excluded.kpz_id, reg_id=excluded.reg_id, enabled=excluded.enabled, cmp=excluded.cmp, \
                       set_lo=excluded.set_lo, set_hi=excluded.set_hi, set_lo_1=excluded.set_lo_1, set_hi_1=excluded.set_hi_1, hysteresis=excluded.hysteresis, \
                       on_delay_sec=excluded.on_delay_sec, off_delay_sec=excluded.off_delay_sec, severity=excluded.severity, \
                       code=excluded.code, message=excluded.message, updated_at=now() \
                     returning id",
                    &[
                        &row.id,
                        &row.kpz_id,
                        &row.reg_id,
                        &row.enabled,
                        &row.cmp,
                        &row.set_lo,
                        &row.set_hi,
                        &row.set_lo_1,
                        &row.set_hi_1,
                        &row.hysteresis,
                        &row.on_delay_sec,
                        &row.off_delay_sec,
                        &row.severity,
                        &row.code,
                        &row.message,
                    ],
                )
                .await?;
            Ok(r.get::<_, i64>(0))
        })
    }

    #[allow(dead_code)]
    pub fn insert_alarm_rule(&self, row: &AlarmRuleRow) -> Result<i64> {
        self.rt.block_on(async {
            let r = self
                .client
                .query_one(
                    "insert into alarm_rule(kpz_id, reg_id, enabled, cmp, set_lo, set_hi, set_lo_1, set_hi_1, hysteresis, on_delay_sec, off_delay_sec, severity, code, message) \
                     values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14) returning id",
                    &[
                        &row.kpz_id,
                        &row.reg_id,
                        &row.enabled,
                        &row.cmp,
                        &row.set_lo,
                        &row.set_hi,
                        &row.set_lo_1,
                        &row.set_hi_1,
                        &row.hysteresis,
                        &row.on_delay_sec,
                        &row.off_delay_sec,
                        &row.severity,
                        &row.code,
                        &row.message,
                    ],
                )
                .await?;
            Ok(r.get::<_, i64>(0))
        })
    }

    #[allow(dead_code)]
    pub fn delete_alarm_rule(&self, id: i64) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute("delete from alarm_rule where id = $1", &[&id])
                .await?;
            Ok(())
        })
    }

    #[allow(dead_code)]
    pub fn get_alarm_state(&self, kpz_id: Option<i32>, limit: i64) -> Result<Vec<AlarmStateRow>> {
        self.rt.block_on(async {
            let rows = if let Some(k) = kpz_id {
                self.client
                    .query(
                        "select s.rule_id, r.kpz_id, r.reg_id, s.active, \
                                to_char(s.active_since,'YYYY-MM-DD HH24:MI:SS.MS') as active_since, \
                                s.last_value, to_char(s.updated_at,'YYYY-MM-DD HH24:MI:SS.MS') as updated_at \
                         from alarm_state s \
                         join alarm_rule r on r.id = s.rule_id \
                         where r.kpz_id = $1 \
                         order by s.updated_at desc \
                         limit $2",
                        &[&k, &limit],
                    )
                    .await?
            } else {
                self.client
                    .query(
                        "select s.rule_id, r.kpz_id, r.reg_id, s.active, \
                                to_char(s.active_since,'YYYY-MM-DD HH24:MI:SS.MS') as active_since, \
                                s.last_value, to_char(s.updated_at,'YYYY-MM-DD HH24:MI:SS.MS') as updated_at \
                         from alarm_state s \
                         join alarm_rule r on r.id = s.rule_id \
                         order by s.updated_at desc \
                         limit $1",
                        &[&limit],
                    )
                    .await?
            };
            let out = rows
                .into_iter()
                .map(|r| AlarmStateRow {
                    rule_id: r.get::<_, i64>(0),
                    kpz_id: r.get::<_, i32>(1),
                    reg_id: r.get::<_, i32>(2),
                    active: r
                        .try_get::<_, bool>(3)
                        .ok()
                        .or_else(|| r.try_get::<_, i16>(3).ok().map(|v| v != 0))
                        .or_else(|| r.try_get::<_, i32>(3).ok().map(|v| v != 0))
                        .unwrap_or(false),
                    active_since: r.try_get::<_, Option<String>>(4).ok().flatten(),
                    last_value: r.try_get::<_, Option<f64>>(5).ok().flatten(),
                    updated_at: r.get::<_, String>(6),
                })
                .collect();
            Ok(out)
        })
    }

    #[allow(dead_code)]
    pub fn get_alarm_events(&self, kpz_id: Option<i32>, limit: i64) -> Result<Vec<AlarmEventRow>> {
        self.rt.block_on(async {
            let rows = if let Some(k) = kpz_id {
                self.client
                    .query(
                        "select id, to_char(ts,'YYYY-MM-DD HH24:MI:SS.MS') as ts, kpz_id, reg_id, rule_id, event, \
                                value, set_lo, set_hi, severity, code, message \
                         from alarm_event where kpz_id = $1 \
                         order by ts desc, id desc limit $2",
                        &[&k, &limit],
                    )
                    .await?
            } else {
                self.client
                    .query(
                        "select id, to_char(ts,'YYYY-MM-DD HH24:MI:SS.MS') as ts, kpz_id, reg_id, rule_id, event, \
                                value, set_lo, set_hi, severity, code, message \
                         from alarm_event \
                         order by ts desc, id desc limit $1",
                        &[&limit],
                    )
                    .await?
            };
            let out = rows
                .into_iter()
                .map(|r| AlarmEventRow {
                    id: r.get::<_, i64>(0),
                    ts: r.get::<_, String>(1),
                    kpz_id: r.get::<_, i32>(2),
                    reg_id: r.get::<_, i32>(3),
                    rule_id: r.get::<_, i64>(4),
                    event: r.get::<_, String>(5),
                    value: r.try_get::<_, Option<f64>>(6).ok().flatten(),
                    set_lo: r.try_get::<_, Option<f64>>(7).ok().flatten(),
                    set_hi: r.try_get::<_, Option<f64>>(8).ok().flatten(),
                    severity: r
                        .try_get::<_, i16>(9)
                        .ok()
                        .or_else(|| r.try_get::<_, i32>(9).ok().map(|v| v as i16))
                        .unwrap_or(1),
                    code: r.try_get::<_, Option<String>>(10).ok().flatten(),
                    message: r.try_get::<_, Option<String>>(11).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }
}
