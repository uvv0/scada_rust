use anyhow::Result;

use crate::db::Db;
use crate::models::{KpzRow, ObjRow, RegEditRow};

#[allow(dead_code)]
impl Db {
    pub fn get_all_kpz(&self) -> Result<Vec<KpzRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, coalesce(name,''), coalesce(rtu,0), coalesce(obj,0), modem, \
                     grups, max_pkt_len, coalesce(start,0), coalesce(t_a::text,''), \
                     coalesce(t_script::text,''), coalesce(en_post, false) \
                     from kpz order by id",
                    &[],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| {
                    let grups = r
                        .try_get::<_, Option<Vec<u8>>>(5)
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| vec![0u8; 64]);
                    KpzRow {
                        id: r.get::<_, i32>(0),
                        name: r.get::<_, String>(1),
                        rtu: r.get::<_, i32>(2),
                        obj: r.get::<_, i32>(3),
                        modem: r.try_get::<_, Option<i32>>(4).ok().flatten(),
                        max_pkt_len: r.try_get::<_, Option<i32>>(6).ok().flatten(),
                        start: r.get::<_, i32>(7),
                        grups,
                        t_a: r.get::<_, String>(8),
                        t_script: r.get::<_, String>(9),
                        en_post: r.get::<_, bool>(10),
                    }
                })
                .collect();
            Ok(out)
        })
    }

    pub fn get_all_obj(&self) -> Result<Vec<ObjRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, coalesce(name,''), ip, port, kanal, speed, stop, parit, bit \
                     from obj order by id",
                    &[],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| {
                    let ip_raw = r.try_get::<_, Option<String>>(2).ok().flatten();
                    let ip = ip_raw
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .and_then(|s| s.parse::<i32>().ok());
                    ObjRow {
                        id: r.get::<_, i32>(0),
                        name: r.get::<_, String>(1),
                        ip_raw,
                        ip,
                        port: r.try_get::<_, Option<i32>>(3).ok().flatten(),
                        kanal: r.try_get::<_, Option<i32>>(4).ok().flatten(),
                        speed: r.try_get::<_, Option<i32>>(5).ok().flatten(),
                        stop: r.try_get::<_, Option<i32>>(6).ok().flatten(),
                        parit: r.try_get::<_, Option<i32>>(7).ok().flatten(),
                        bit: r.try_get::<_, Option<i32>>(8).ok().flatten(),
                    }
                })
                .collect();
            Ok(out)
        })
    }

    pub fn get_scheduler_modbus_a_timeout_ms(&self) -> Result<Option<i64>> {
        self.rt.block_on(async {
            let row = self
                .client
                .query_opt(
                    "select modbus_a_timeout_ms from public.scheduler_runtime_cfg order by id limit 1",
                    &[],
                )
                .await?;
            Ok(row.and_then(|r| r.try_get::<_, Option<i64>>(0).ok().flatten()))
        })
    }

    pub fn update_reg_val(&self, reg_id: i32, val: f64) -> Result<()> {
        let sval = val.to_string();
        self.rt.block_on(async {
            self.client
                .execute("update reg set val = $1 where id = $2", &[&sval, &reg_id])
                .await?;
            Ok(())
        })
    }

    pub fn update_reg_val_checked(&self, reg_id: i32, val: f64) -> Result<u64> {
        let sval = val.to_string();
        self.rt.block_on(async {
            let n = self
                .client
                .execute("update reg set val = $1 where id = $2", &[&sval, &reg_id])
                .await?;
            Ok(n)
        })
    }

    pub fn update_kpz_meta(
        &self,
        id: i32,
        start: i32,
        t_a: Option<i32>,
        t_script: Option<i32>,
    ) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute(
                    "update kpz \
                     set start = $1, \
                         t_a = $2, \
                         t_script = $3 \
                     where id = $4",
                    &[&start, &t_a, &t_script, &id],
                )
                .await?;
            Ok(())
        })
    }

    pub fn update_kpz_full(
        &self,
        id: i32,
        name: &str,
        rtu: i32,
        obj: i32,
        modem: Option<i32>,
        max_pkt_len: i32,
        start: i32,
        t_a: Option<i32>,
        t_script: Option<i32>,
        en_post: bool,
    ) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute(
                    "update kpz \
                     set name = $1, \
                         rtu = $2, \
                         obj = $3, \
                         modem = $4, \
                         max_pkt_len = $5, \
                         start = $6, \
                         t_a = $7, \
                         t_script = $8, \
                         en_post = $9 \
                     where id = $10",
                    &[
                        &name,
                        &rtu,
                        &obj,
                        &modem,
                        &max_pkt_len,
                        &start,
                        &t_a,
                        &t_script,
                        &en_post,
                        &id,
                    ],
                )
                .await?;
            Ok(())
        })
    }

    #[allow(dead_code)]
    pub fn upsert_test_kpz_range(
        &self,
        id_start: i32,
        id_end: i32,
        obj_id: i32,
        modem_start: i32,
        max_pkt_len: i32,
    ) -> Result<u64> {
        self.rt.block_on(async {
            let n = self
                .client
                .execute(
                    "insert into kpz(id, name, rtu, obj, modem, grups, max_pkt_len, start, t_a, t_script)
                     select gs,
                            format('test_kpz_%s', gs),
                            gs,
                            $3::int,
                            ($4::int + (gs - $1::int)),
                            decode(repeat('00', 64), 'hex'),
                            $5::int,
                            0,
                            0,
                            0
                     from generate_series($1::int, $2::int) as gs
                     on conflict (id) do update
                     set name = excluded.name,
                         rtu = excluded.rtu,
                         obj = excluded.obj,
                         modem = excluded.modem,
                         max_pkt_len = excluded.max_pkt_len",
                    &[&id_start, &id_end, &obj_id, &modem_start, &max_pkt_len],
                )
                .await?;
            Ok(n)
        })
    }

    pub fn set_kpz_start_range(&self, id_start: i32, id_end: i32, start: bool) -> Result<u64> {
        let start_i = if start { 1 } else { 0 };
        self.rt.block_on(async {
            let n = self
                .client
                .execute(
                    "update kpz set start = $3 where id between $1 and $2",
                    &[&id_start, &id_end, &start_i],
                )
                .await?;
            Ok(n)
        })
    }

    pub fn set_kpz_timing_range(
        &self,
        id_start: i32,
        id_end: i32,
        t_a: Option<i32>,
        t_script: Option<i32>,
    ) -> Result<u64> {
        self.rt.block_on(async {
            let n = self
                .client
                .execute(
                    "update kpz set t_a = $3, t_script = $4 where id between $1 and $2",
                    &[&id_start, &id_end, &t_a, &t_script],
                )
                .await?;
            Ok(n)
        })
    }

    pub fn update_obj(
        &self,
        id: i32,
        name: &str,
        ip: Option<i32>,
        port: Option<i32>,
        kanal: Option<i32>,
        speed: Option<i32>,
        stop: Option<i32>,
        parit: Option<i32>,
        bit: Option<i32>,
    ) -> Result<()> {
        let ip_text: Option<String> = ip.map(|v| v.to_string());
        self.rt.block_on(async {
            self.client
                .execute(
                    "update obj \
                     set name = $1, \
                         ip = $2, \
                         port = $3, \
                         kanal = $4, \
                         speed = $5, \
                         stop = $6, \
                         parit = $7, \
                         bit = $8 \
                     where id = $9",
                    &[&name, &ip_text, &port, &kanal, &speed, &stop, &parit, &bit, &id],
                )
                .await?;
            Ok(())
        })
    }

    pub fn get_all_reg_edit(&self) -> Result<Vec<RegEditRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, coalesce(name,''), coalesce(mb,0), n_mb, coalesce(tip,0), bits, grup, \
                     case when a_en::text in ('1','t','true','T','TRUE') then 1 else 0 end as a_en_i, \
                     coalesce(a_no_write,0) \
                     from reg order by id",
                    &[],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| RegEditRow {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                    mb: r.get::<_, i32>(2),
                    n_mb: r.try_get::<_, Option<i32>>(3).ok().flatten(),
                    tip: r.get::<_, i32>(4),
                    bits: r.try_get::<_, Option<i32>>(5).ok().flatten(),
                    grup: r.try_get::<_, Option<i32>>(6).ok().flatten(),
                    a_en: r.get::<_, i32>(7) != 0,
                    a_no_write: r.get::<_, i32>(8),
                })
                .collect();
            Ok(out)
        })
    }

    pub fn update_reg_edit(
        &self,
        id: i32,
        name: &str,
        mb: i32,
        n_mb: Option<i32>,
        tip: i32,
        bits: Option<i32>,
        grup: i32,
        a_en: bool,
        a_no_write: i32,
    ) -> Result<()> {
        let a_no_write_i16 = a_no_write.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        self.rt.block_on(async {
            self.client
                .execute(
                    "insert into reg(id, name, mb, n_mb, tip, bits, grup, a_en, a_no_write) \
                     values($9, $1, $2, $3, $4, $5, $6, $7, $8) \
                     on conflict (id) do update \
                     set name = excluded.name, \
                         mb = excluded.mb, \
                         n_mb = excluded.n_mb, \
                         tip = excluded.tip, \
                         bits = excluded.bits, \
                         grup = excluded.grup, \
                         a_en = excluded.a_en, \
                         a_no_write = excluded.a_no_write",
                    &[&name, &mb, &n_mb, &tip, &bits, &grup, &a_en, &a_no_write_i16, &id],
                )
                .await?;
            Ok(())
        })
    }

    pub fn update_kpz_grups(&self, id: i32, grups: &[u8]) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute("update kpz set grups = $1 where id = $2", &[&grups, &id])
                .await?;
            Ok(())
        })
    }

    pub fn write_reg_command_and_verify(&self, reg_id: i32, val: f64) -> Result<Option<f64>> {
        self.rt.block_on(async {
            self.client
                .execute("update reg set val = $1 where id = $2", &[&val, &reg_id])
                .await?;
            let row = self
                .client
                .query_opt("select val from reg where id = $1", &[&reg_id])
                .await?;
            let got = row.and_then(|r| r.try_get::<_, Option<f64>>(0).ok().flatten());
            Ok(got)
        })
    }
}
