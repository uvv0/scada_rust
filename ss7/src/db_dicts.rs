use anyhow::Result;

use crate::db::Db;
use crate::models::DictItemRow;

impl Db {
    pub fn get_items(&self, table: &str) -> Result<Vec<DictItemRow>> {
        let sql = match table {
            "ip" | "port" | "speed" | "parit" | "bit" | "stop" | "kanal" | "grup" | "n_mb"
            | "tip" | "bits" | "c" => {
                format!("select id, coalesce(name,'') from {} order by id", table)
            }
            _ => return Err(anyhow::anyhow!("table not allowed: {table}")),
        };
        self.rt.block_on(async {
            let rows = self.client.query(&sql, &[]).await?;
            let out = rows
                .into_iter()
                .map(|r| DictItemRow {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                })
                .collect();
            Ok(out)
        })
    }

    #[allow(dead_code)]
    pub fn upsert_item(&self, table: &str, id: i32, name: &str) -> Result<()> {
        let sql = match table {
            "ip" | "port" | "speed" | "parit" | "bit" | "stop" | "kanal" | "grup" | "n_mb"
            | "tip" | "bits" | "c" => {
                format!(
                    "insert into {}(id, name) values($1, $2) \
                     on conflict (id) do update set name = excluded.name",
                    table
                )
            }
            _ => return Err(anyhow::anyhow!("table not allowed: {table}")),
        };
        self.rt.block_on(async {
            self.client.execute(&sql, &[&id, &name]).await?;
            Ok(())
        })
    }

    #[allow(dead_code)]
    pub fn delete_item(&self, table: &str, id: i32) -> Result<()> {
        let sql = match table {
            "ip" | "port" | "speed" | "parit" | "bit" | "stop" | "kanal" | "grup" | "n_mb"
            | "tip" | "bits" | "c" => format!("delete from {} where id = $1", table),
            _ => return Err(anyhow::anyhow!("table not allowed: {table}")),
        };
        self.rt.block_on(async {
            self.client.execute(&sql, &[&id]).await?;
            Ok(())
        })
    }
}
