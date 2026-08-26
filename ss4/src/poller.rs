//! Вспомогательные структуры контекста опроса и преобразование конфигурации КПЗ в ConnInfo.

use std::collections::HashMap;

use anyhow::Result;

use crate::db_queries::build_conn;
use crate::types::{ConnInfo, KpzRow, ObjRow};

#[allow(dead_code)]
/// Контекст опроса одного КПЗ: его конфигурация и рассчитанные сетевые реквизиты.
pub struct PollerContext {
    pub kpz: KpzRow,
    pub conn: ConnInfo,
}

#[allow(dead_code)]
/// Кэш справочников, используемый для быстрого построения `ConnInfo` по `kpz_id`.
pub struct PollerCache {
    pub kpz_by_id: HashMap<i32, KpzRow>,
    pub obj_by_id: HashMap<i32, ObjRow>,
    pub ip_by_id: HashMap<i32, String>,
    pub port_by_id: HashMap<i32, u16>,
}

impl PollerCache {
    #[allow(dead_code)]
    /// Строит `ConnInfo` для выбранного КПЗ по текущим кэшам `kpz/obj/ip/port`.
    ///
    /// # Parameters
    /// - `kpz_id`: идентификатор КПЗ.
    ///
    /// # Returns
    /// - `Ok(ConnInfo)`, если все ссылки разрешены.
    /// - `Err(...)`, если КПЗ или связанные данные отсутствуют/некорректны.
    pub fn build_conn_for_kpz(&self, kpz_id: i32) -> Result<ConnInfo> {
        let kpz = self
            .kpz_by_id
            .get(&kpz_id)
            .ok_or_else(|| anyhow::anyhow!("kpz={} not found", kpz_id))?;
        build_conn(kpz, &self.obj_by_id, &self.ip_by_id, &self.port_by_id)
    }
}
