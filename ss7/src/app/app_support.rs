use std::collections::HashMap;
use std::net::IpAddr;

use crate::app::Ss7App;
use crate::app_io::IoConn;
use crate::db::Db;

impl Ss7App {
    pub(crate) fn load_dict_map(db: &Db, table: &str) -> HashMap<i32, String> {
        db.get_items(table)
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.id, r.name))
            .collect()
    }

    fn dict_num(map: &HashMap<i32, String>, id: Option<i32>, default: i32) -> i32 {
        let Some(idv) = id else { return default };
        map.get(&idv)
            .and_then(|s| s.trim().parse::<i32>().ok())
            .unwrap_or(default)
    }

    pub(crate) fn build_io_conn(&self) -> Result<IoConn, String> {
        let kpz_id = self.selected_kpz.ok_or_else(|| "No KPZ selected".to_string())?;
        let kpz = self
            .kpz
            .iter()
            .find(|k| k.id == kpz_id)
            .ok_or_else(|| format!("KPZ {} not found", kpz_id))?;
        let obj = self
            .obj_rows
            .iter()
            .find(|o| o.id == kpz.obj)
            .ok_or_else(|| format!("OBJ {} not found", kpz.obj))?;

        let ip = obj
            .ip
            .and_then(|id| self.ref_ip.get(&id).cloned())
            .or_else(|| {
                obj.ip_raw
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty() && s.parse::<i32>().is_err())
                    .map(ToOwned::to_owned)
            })
            .or_else(|| {
                let name = obj.name.trim();
                if name.parse::<IpAddr>().is_ok() {
                    Some(name.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "127.0.0.1".to_string());
        let port = obj
            .port
            .and_then(|id| self.ref_port.get(&id).cloned())
            .and_then(|s| s.trim().parse::<u16>().ok())
            .unwrap_or(5100);

        Ok(IoConn {
            ip,
            port,
            rtu: kpz.rtu.max(1) as u16,
            modem: kpz.modem.unwrap_or(50002).max(0) as u16,
            timeout_ms: self.modbus_a_timeout_ms,
            kan: Self::dict_num(&self.ref_kanal, obj.kanal, 3).clamp(0, 255) as u8,
            speed: Self::dict_num(&self.ref_speed, obj.speed, 8).clamp(0, 255) as u8,
            stop: Self::dict_num(&self.ref_stop, obj.stop, 0).clamp(0, 255) as u8,
            par: Self::dict_num(&self.ref_parit, obj.parit, 2).clamp(0, 255) as u8,
            data: Self::dict_num(&self.ref_bit, obj.bit, 8).clamp(0, 255) as u8,
            max_pkt_len: kpz.max_pkt_len.unwrap_or(800).max(256) as usize,
        })
    }

    pub(crate) fn selected_kpz_name(&self) -> String {
        self.selected_kpz
            .and_then(|id| self.kpz.iter().find(|k| k.id == id))
            .map(|k| k.name.clone())
            .unwrap_or_default()
    }

    pub(crate) fn kpz_ip_modem_for(&self, kpz_sel: Option<i32>) -> (String, String) {
        let Some(kpz_id) = kpz_sel else {
            return ("-".to_string(), "-".to_string());
        };
        let Some(kpz) = self.kpz.iter().find(|k| k.id == kpz_id) else {
            return ("-".to_string(), "-".to_string());
        };
        let Some(obj) = self.obj_rows.iter().find(|o| o.id == kpz.obj) else {
            return ("-".to_string(), kpz.modem.unwrap_or(50002).to_string());
        };
        let ip = obj
            .ip
            .and_then(|id| self.ref_ip.get(&id).cloned())
            .or_else(|| {
                obj.ip_raw
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty() && s.parse::<i32>().is_err())
                    .map(ToOwned::to_owned)
            })
            .or_else(|| {
                let name = obj.name.trim();
                if name.parse::<IpAddr>().is_ok() {
                    Some(name.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "-".to_string());
        let modem = kpz.modem.unwrap_or(50002).to_string();
        (ip, modem)
    }

    #[allow(dead_code)]
    pub(crate) fn reload_refs(&mut self) {
        match self.db.get_all_kpz() {
            Ok(v) => self.kpz = v,
            Err(e) => self.err = Some(format!("get_all_kpz failed: {e}")),
        }
        match self.db.get_all_groups() {
            Ok(v) => self.groups = v,
            Err(e) => self.err = Some(format!("get_all_groups failed: {e}")),
        }
        match self.db.get_all_obj() {
            Ok(v) => self.obj_rows = v,
            Err(e) => self.err = Some(format!("get_all_obj failed: {e}")),
        }
        self.ref_ip = Self::load_dict_map(&self.db, "ip");
        self.ref_port = Self::load_dict_map(&self.db, "port");
        self.ref_speed = Self::load_dict_map(&self.db, "speed");
        self.ref_parit = Self::load_dict_map(&self.db, "parit");
        self.ref_bit = Self::load_dict_map(&self.db, "bit");
        self.ref_stop = Self::load_dict_map(&self.db, "stop");
        self.ref_kanal = Self::load_dict_map(&self.db, "kanal");
        self.ref_n_mb = Self::load_dict_map(&self.db, "n_mb");
    }
}
