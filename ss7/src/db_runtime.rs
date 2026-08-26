use anyhow::Result;

use crate::db::Db;
use crate::models::{ArxPointRow, ArxSeriesRow, ElamRow, GScriptRow, PollLogRow};

impl Db {
    #[allow(dead_code)]
    pub fn get_poll_log(&self, kpz_id: Option<i32>, limit: i64) -> Result<Vec<PollLogRow>> {
        self.rt.block_on(async {
            let rows = if let Some(k) = kpz_id {
                self.client
                    .query(
                        "select to_char(l.ts,'YYYY-MM-DD HH24:MI:SS.MS') as ts, l.kpz_id, \
                         coalesce(l.kind,''), coalesce(l.msg,'') \
                         from poll_log l where l.kpz_id = $1 \
                         order by l.ts desc, l.id desc limit $2",
                        &[&k, &limit],
                    )
                    .await?
            } else {
                self.client
                    .query(
                        "select to_char(l.ts,'YYYY-MM-DD HH24:MI:SS.MS') as ts, l.kpz_id, \
                         coalesce(l.kind,''), coalesce(l.msg,'') \
                         from poll_log l \
                         order by l.ts desc, l.id desc limit $1",
                        &[&limit],
                    )
                    .await?
            };

            let out = rows
                .into_iter()
                .map(|r| PollLogRow {
                    ts: r.get::<_, String>(0),
                    kpz_id: r.try_get::<_, Option<i32>>(1).ok().flatten(),
                    kind: r.get::<_, String>(2),
                    msg: r.get::<_, String>(3),
                })
                .collect();
            Ok(out)
        })
    }

    #[allow(dead_code)]
    pub fn get_last_elam(&self, kpz_id: Option<i32>, limit: i64) -> Result<Vec<ElamRow>> {
        self.rt.block_on(async {
            let rows = if let Some(k) = kpz_id {
                self.client
                    .query(
                        "select e.id, to_char(e.ts,'YYYY-MM-DD HH24:MI:SS.MS') as ts, \
                         e.kpz_id, e.group_id, coalesce(e.status,''), e.duration_ms, \
                         e.func, e.addr_human, e.count_words, e.req, e.resp \
                         from elam e where e.kpz_id = $1 \
                         order by e.ts desc, e.id desc limit $2",
                        &[&k, &limit],
                    )
                    .await?
            } else {
                self.client
                    .query(
                        "select e.id, to_char(e.ts,'YYYY-MM-DD HH24:MI:SS.MS') as ts, \
                         e.kpz_id, e.group_id, coalesce(e.status,''), e.duration_ms, \
                         e.func, e.addr_human, e.count_words, e.req, e.resp \
                         from elam e \
                         order by e.ts desc, e.id desc limit $1",
                        &[&limit],
                    )
                    .await?
            };

            let out = rows
                .into_iter()
                .map(|r| ElamRow {
                    id: r.get::<_, i64>(0),
                    ts: r.get::<_, String>(1),
                    kpz_id: r.get::<_, i32>(2),
                    group_id: r.try_get::<_, Option<i32>>(3).ok().flatten(),
                    status: r.get::<_, String>(4),
                    duration_ms: r.try_get::<_, Option<i32>>(5).ok().flatten(),
                    func: r.try_get::<_, Option<i32>>(6).ok().flatten(),
                    addr_human: r.try_get::<_, Option<i32>>(7).ok().flatten(),
                    count_words: r.try_get::<_, Option<i32>>(8).ok().flatten(),
                    req: r.try_get::<_, Vec<u8>>(9).unwrap_or_default(),
                    resp: r.try_get::<_, Option<Vec<u8>>>(10).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }

    #[allow(dead_code)]
    pub fn get_g_script(&self, grup: i32) -> Result<Option<GScriptRow>> {
        self.rt.block_on(async {
            let row = self
                .client
                .query_opt(
                    "select grup, elam, max, max_k, pre_src, post_src, en, ver \
                     from g_script where grup = $1 limit 1",
                    &[&grup],
                )
                .await?;
            let Some(r) = row else {
                return Ok(None);
            };
            let elam = r
                .try_get::<_, Option<i16>>(1)
                .ok()
                .flatten()
                .map(|v| v as i32)
                .unwrap_or(0);
            let max_words = r.try_get::<_, Option<i32>>(2).ok().flatten().unwrap_or(800);
            let max_k = r.try_get::<_, Option<i32>>(3).ok().flatten().unwrap_or(2);
            let pre_src = r.try_get::<_, Option<String>>(4).ok().flatten().unwrap_or_default();
            let post_src = r.try_get::<_, Option<String>>(5).ok().flatten().unwrap_or_default();
            let en = r.try_get::<_, Option<bool>>(6).ok().flatten().unwrap_or(true);
            let ver = r.try_get::<_, Option<i32>>(7).ok().flatten().unwrap_or(1);
            Ok(Some(GScriptRow {
                grup: r.get::<_, i32>(0),
                elam,
                max_words,
                max_k,
                pre_src,
                post_src,
                en,
                ver,
            }))
        })
    }

    #[allow(dead_code)]
    pub fn list_g_script_groups(&self) -> Result<Vec<i32>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query("select grup from g_script order by grup", &[])
                .await?;
            Ok(rows.into_iter().map(|r| r.get::<_, i32>(0)).collect())
        })
    }

    #[allow(dead_code)]
    pub fn get_last_arx_vals(&self, kpz_id: i32) -> Result<std::collections::HashMap<i32, f64>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select distinct on (reg_id) reg_id, val_num \
                     from arx_val \
                     where kpz_id = $1 and val_num is not null \
                     order by reg_id, ts_unix desc",
                    &[&kpz_id],
                )
                .await?;
            let mut out = std::collections::HashMap::new();
            for r in rows {
                let reg_id = r.get::<_, i32>(0);
                let val = r.get::<_, f64>(1);
                out.insert(reg_id, val);
            }
            Ok(out)
        })
    }

    pub fn list_g_scripts(&self) -> Result<Vec<GScriptRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select grup, elam, max, max_k, pre_src, post_src, en, ver \
                     from g_script order by grup",
                    &[],
                )
                .await?;
            let mut out = Vec::with_capacity(rows.len());
            for r in rows {
                out.push(GScriptRow {
                    grup: r.get::<_, i32>(0),
                    elam: r.try_get::<_, Option<i16>>(1).ok().flatten().map(|v| v as i32).unwrap_or(0),
                    max_words: r.try_get::<_, Option<i32>>(2).ok().flatten().unwrap_or(800),
                    max_k: r.try_get::<_, Option<i32>>(3).ok().flatten().unwrap_or(2),
                    pre_src: r.try_get::<_, Option<String>>(4).ok().flatten().unwrap_or_default(),
                    post_src: r.try_get::<_, Option<String>>(5).ok().flatten().unwrap_or_default(),
                    en: r.try_get::<_, Option<bool>>(6).ok().flatten().unwrap_or(true),
                    ver: r.try_get::<_, Option<i32>>(7).ok().flatten().unwrap_or(1),
                });
            }
            Ok(out)
        })
    }

    #[allow(dead_code)]
    pub fn get_arx_series(
        &self,
        kpz_id: i32,
        reg_ids: &[i32],
        limit: i64,
        window_sec: i64,
    ) -> Result<Vec<ArxSeriesRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select reg_id, ts_unix, val_num \
                     from arx_val \
                     where kpz_id = $1 and reg_id = any($2) and val_num is not null \
                       and ts_unix >= (extract(epoch from now())::bigint - $4) \
                       and ts_unix <= (extract(epoch from now())::bigint + 86400) \
                     order by ts_unix desc \
                     limit $3",
                    &[&kpz_id, &reg_ids, &limit, &window_sec],
                )
                .await?;

            let mut by_reg: std::collections::BTreeMap<i32, Vec<ArxPointRow>> =
                std::collections::BTreeMap::new();
            for r in rows {
                let reg_id = r.get::<_, i32>(0);
                let ts_unix = r.get::<_, i64>(1);
                let val_num = r.get::<_, f64>(2);
                by_reg
                    .entry(reg_id)
                    .or_default()
                    .push(ArxPointRow { ts_unix, val_num });
            }

            let mut out = Vec::new();
            for (reg_id, mut points) in by_reg {
                points.reverse();
                out.push(ArxSeriesRow { reg_id, points });
            }
            Ok(out)
        })
    }

    #[allow(dead_code)]
    pub fn upsert_g_script(&self, row: &GScriptRow) -> Result<()> {
        let elam_i16 = row.elam.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        self.rt.block_on(async {
            self.client
                .execute(
                    "insert into g_script(grup, elam, max, max_k, pre_src, post_src, en, ver) \
                     values($1,$2,$3,$4,$5,$6,$7,$8) \
                     on conflict (grup) do update set \
                     elam=excluded.elam, max=excluded.max, max_k=excluded.max_k, \
                     pre_src=excluded.pre_src, post_src=excluded.post_src, \
                     en=excluded.en, ver=excluded.ver, updated_at=now()",
                    &[
                        &row.grup,
                        &elam_i16,
                        &row.max_words,
                        &row.max_k,
                        &row.pre_src,
                        &row.post_src,
                        &row.en,
                        &row.ver,
                    ],
                )
                .await?;
            Ok(())
        })
    }
}
