use anyhow::Result;

use crate::db::Db;
use crate::models::{GroupRow, KpzIoRow, RegRow};

impl Db {
    pub fn get_all_groups(&self) -> Result<Vec<GroupRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query("select id, coalesce(name,'') from grup order by id", &[])
                .await?;
            let out = rows
                .into_iter()
                .map(|r| GroupRow {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                })
                .collect();
            Ok(out)
        })
    }

    #[allow(dead_code)]
    pub fn get_regs_for_group(&self, group_id: i32) -> Result<Vec<RegRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, coalesce(name,''), coalesce(grup,0), coalesce(mb,0), n_mb, coalesce(tip,0), bits \
                     from reg where grup = $1 order by mb asc nulls last, id asc",
                    &[&group_id],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| RegRow {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                    grup: r.get::<_, i32>(2),
                    mb: r.get::<_, i32>(3),
                    n_mb: r.try_get::<_, Option<i32>>(4).ok().flatten(),
                    tip: r.get::<_, i32>(5),
                    bits: r.try_get::<_, Option<i32>>(6).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }

    #[allow(dead_code)]
    pub fn get_kpz_io_rows(&self, kpz_id: i32, group_id: i32, n_mb: i32) -> Result<Vec<KpzIoRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select r.id, coalesce(r.name,''), coalesce(r.mb,0), coalesce(r.tip,0), r.bits, \
                            r.val::double precision as reg_val, \
                            (select av.val_num \
                               from arx_val av \
                              where av.kpz_id = $1 and av.reg_id = r.id and av.val_num is not null \
                              order by av.ts_unix desc \
                              limit 1) as last_val \
                     from reg r \
                     where r.grup = $2 and coalesce(r.n_mb, 0) = $3 \
                     order by r.mb asc nulls last, r.id asc",
                    &[&kpz_id, &group_id, &n_mb],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| KpzIoRow {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                    mb: r.get::<_, i32>(2),
                    tip: r.get::<_, i32>(3),
                    bits: r.try_get::<_, Option<i32>>(4).ok().flatten(),
                    reg_val: r.try_get::<_, Option<f64>>(5).ok().flatten(),
                    last_val: r.try_get::<_, Option<f64>>(6).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }

    pub fn get_regs_by_groups(&self, group_ids: &[i32]) -> Result<Vec<RegRow>> {
        if group_ids.is_empty() {
            return Ok(Vec::new());
        }
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, coalesce(name,''), coalesce(grup,0), coalesce(mb,0), n_mb, coalesce(tip,0), bits \
                     from reg \
                     where grup = any($1) \
                     order by grup, mb asc nulls last, id asc",
                    &[&group_ids],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| RegRow {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                    grup: r.get::<_, i32>(2),
                    mb: r.get::<_, i32>(3),
                    n_mb: r.try_get::<_, Option<i32>>(4).ok().flatten(),
                    tip: r.get::<_, i32>(5),
                    bits: r.try_get::<_, Option<i32>>(6).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }

    pub fn get_groups_by_reg_ids(&self, reg_ids: &[i32]) -> Result<Vec<i32>> {
        if reg_ids.is_empty() {
            return Ok(Vec::new());
        }
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select distinct coalesce(grup, 0) as grup
                     from reg
                     where id = any($1) and coalesce(grup, 0) > 0
                     order by grup",
                    &[&reg_ids],
                )
                .await?;
            let out = rows.into_iter().map(|r| r.get::<_, i32>(0)).collect();
            Ok(out)
        })
    }

    pub fn get_reg_live_values(&self, kpz_id: i32, reg_ids: &[i32]) -> Result<Vec<(i32, Option<f64>)>> {
        if reg_ids.is_empty() {
            return Ok(Vec::new());
        }
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select r.id,
                            coalesce(
                                case
                                    when r.val is null then null
                                    when btrim(r.val) = '' then null
                                    when replace(btrim(r.val), ',', '.') ~ '^[+-]?([0-9]+(\\.[0-9]+)?|\\.[0-9]+)$'
                                        then replace(btrim(r.val), ',', '.')::double precision
                                    else null
                                end,
                                (
                                    select av.val_num
                                    from arx_val av
                                    where av.kpz_id = $1 and av.reg_id = r.id
                                    order by av.ts_unix desc
                                    limit 1
                                )
                            ) as v_num
                     from reg r
                     where r.id = any($2)
                     order by r.id",
                    &[&kpz_id, &reg_ids],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| {
                    (
                        r.get::<_, i32>(0),
                        r.try_get::<_, Option<f64>>(1).ok().flatten(),
                    )
                })
                .collect();
            Ok(out)
        })
    }
}
