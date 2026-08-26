use std::env;
use anyhow::{Result, anyhow};
use tokio::runtime::Runtime;
use tokio_postgres::{Client, NoTls};

use crate::db_config::{PG_DB, PG_HOST, PG_PORT, load_file_config};
use crate::db_schema;

// DB methods are split into focused modules to keep this entry point limited to
// connection setup, migrations and integration tests.
#[path = "db_accounts.rs"]
mod db_accounts;
#[path = "db_dicts.rs"]
mod db_dicts;
#[path = "db_regs.rs"]
mod db_regs;
#[path = "db_ui_windows.rs"]
mod db_ui_windows;
#[path = "db_kp_templates.rs"]
mod db_kp_templates;
#[path = "db_alarm.rs"]
mod db_alarm;
#[path = "db_runtime.rs"]
mod db_runtime;
#[path = "db_core_ops.rs"]
mod db_core_ops;

pub struct Db {
    rt: Runtime,
    client: Client,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_db_url() -> anyhow::Result<String> {
        env::var("TEST_DB_URL")
            .map_err(|_| anyhow!("TEST_DB_URL is required for db integration tests"))
    }

    fn now_marker() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    #[test]
    #[ignore = "requires TEST_DB_URL"]
    fn db_integration_ui_window_binding_text_roundtrip() -> anyhow::Result<()> {
        let url = test_db_url()?;
        let rt = Runtime::new()?;
        rt.block_on(async move {
            let (client, conn) = tokio_postgres::connect(&url, NoTls).await?;
            tokio::spawn(async move {
                let _ = conn.await;
            });

            db_schema::apply_schema_migrations(&client).await?;

            let kpz = client.query_opt("select id from public.kpz order by id limit 1", &[]).await?;
            let reg = client.query_opt("select id from public.reg order by id limit 1", &[]).await?;
            let (Some(kpz), Some(reg)) = (kpz, reg) else {
                return Ok(());
            };
            let kpz_id: i32 = kpz.get(0);
            let reg_id: i32 = reg.get(0);

            let code = format!("itest_{}", now_marker());
            let w = client
                .query_one(
                    "insert into ui.kpz_window(kpz_id, code, title, description, is_active) \
                     values ($1,$2,$3,$4,true) returning id",
                    &[&kpz_id, &code, &"it window", &Some("it desc")],
                )
                .await?;
            let window_id: i64 = w.get(0);

            client.execute(
                "insert into ui.kpz_window_reg_binding(window_id, reg_id, pos, x, y, w, h, visible, writable, label_override, unit, fmt) \
                 values ($1,$2,10,20,30,140,40,true,false,$3,$4,$5)",
                &[&window_id, &reg_id, &Some("it_label"), &Some("u"), &Some("0.0")],
            ).await?;
            client.execute(
                "insert into ui.kpz_window_text_item(window_id, pos, x, y, w, h, visible, text) \
                 values ($1,20,40,50,200,30,true,$2)",
                &[&window_id, &"IT TEXT"],
            ).await?;

            let c1 = client
                .query_one(
                    "select count(*) from ui.kpz_window_reg_binding where window_id=$1",
                    &[&window_id],
                )
                .await?;
            let c2 = client
                .query_one(
                    "select count(*) from ui.kpz_window_text_item where window_id=$1",
                    &[&window_id],
                )
                .await?;
            let bcnt: i64 = c1.get(0);
            let tcnt: i64 = c2.get(0);
            assert_eq!(bcnt, 1);
            assert_eq!(tcnt, 1);

            client
                .execute("delete from ui.kpz_window where id=$1", &[&window_id])
                .await?;
            Ok(())
        })
    }

    #[test]
    #[ignore = "requires TEST_DB_URL"]
    fn db_integration_ui_template_from_window_roundtrip() -> anyhow::Result<()> {
        let url = test_db_url()?;
        let rt = Runtime::new()?;
        rt.block_on(async move {
            let (client, conn) = tokio_postgres::connect(&url, NoTls).await?;
            tokio::spawn(async move {
                let _ = conn.await;
            });

            db_schema::apply_schema_migrations(&client).await?;

            let kpz = client.query_opt("select id from public.kpz order by id limit 1", &[]).await?;
            let reg = client.query_opt("select id from public.reg order by id limit 1", &[]).await?;
            let (Some(kpz), Some(reg)) = (kpz, reg) else {
                return Ok(());
            };
            let kpz_id: i32 = kpz.get(0);
            let reg_id: i32 = reg.get(0);

            let marker = now_marker();
            let window_code = format!("itw_{}", marker);
            let template_code = format!("itt_{}", marker);

            let w = client
                .query_one(
                    "insert into ui.kpz_window(kpz_id, code, title, is_active) \
                     values ($1,$2,$3,true) returning id",
                    &[&kpz_id, &window_code, &"it window"],
                )
                .await?;
            let window_id: i64 = w.get(0);
            client
                .execute(
                    "insert into ui.kpz_window_reg_binding(window_id, reg_id, pos) values ($1,$2,10)",
                    &[&window_id, &reg_id],
                )
                .await?;

            let t = client
                .query_one(
                    "insert into ui.kpz_window_template(code, title, source_window_id, is_active) \
                     values ($1,$2,$3,true) returning id",
                    &[&template_code, &"it template", &window_id],
                )
                .await?;
            let template_id: i64 = t.get(0);
            client
                .execute(
                    "insert into ui.kpz_window_template_binding(template_id, reg_id, pos) values ($1,$2,10)",
                    &[&template_id, &reg_id],
                )
                .await?;

            let c = client
                .query_one(
                    "select count(*) from ui.kpz_window_template_binding where template_id=$1",
                    &[&template_id],
                )
                .await?;
            let cnt: i64 = c.get(0);
            assert_eq!(cnt, 1);

            client
                .execute("delete from ui.kpz_window_template where id=$1", &[&template_id])
                .await?;
            client
                .execute("delete from ui.kpz_window where id=$1", &[&window_id])
                .await?;
            Ok(())
        })
    }
}

#[allow(dead_code)]
impl Db {
    pub fn connect_from_env() -> Result<Self> {
        let host_env = env::var("PG_HOST").ok();
        let port_env = env::var("PG_PORT").ok();
        let db_env = env::var("PG_DB").ok();
        let user_env = env::var("PG_USER").ok();
        let pass_env = env::var("PG_PASS").ok();

        let (host, port, db, user, pass) = if let (
            Some(host),
            Some(port_raw),
            Some(db),
            Some(user),
            Some(pass),
        ) = (host_env, port_env, db_env, user_env, pass_env)
        {
            let port = port_raw
                .parse::<u16>()
                .map_err(|_| anyhow!("PG_PORT must be valid u16"))?;
            (host, port, db, user, pass)
        } else {
            let cfg = load_file_config()?;
            let host = cfg.pg_host.unwrap_or_else(|| PG_HOST.to_string());
            let port = cfg.pg_port.unwrap_or(PG_PORT);
            let db = cfg.pg_db.unwrap_or_else(|| PG_DB.to_string());
            (host, port, db, cfg.pg_user, cfg.pg_pass)
        };

        let rt = Runtime::new()?;
        let conn_str = format!(
            "host={} port={} user={} password={} dbname={}",
            host, port, user, pass, db
        );
        let client = rt.block_on(async {
            let (client, conn) = tokio_postgres::connect(&conn_str, NoTls).await?;
            tokio::spawn(async move {
                if let Err(e) = conn.await {
                    eprintln!("db connection error: {e}");
                }
            });
            db_schema::apply_schema_migrations(&client).await?;
            Result::<Client>::Ok(client)
        })?;

        Ok(Self { rt, client })
    }

    pub fn apply_schema_migrations(&self) -> Result<()> {
        self.rt
            .block_on(async { db_schema::apply_schema_migrations(&self.client).await })
    }

}
