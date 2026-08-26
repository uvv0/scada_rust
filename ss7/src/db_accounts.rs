use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};

use crate::db::Db;
use crate::models::WebAccountRow;

fn hash_web_password(password_salt: &str, password: &str) -> anyhow::Result<String> {
    let salt = SaltString::encode_b64(password_salt.as_bytes())
        .map_err(|e| anyhow::anyhow!("invalid password salt: {e}"))?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("failed to hash password: {e}"))?;
    Ok(hash.to_string())
}

fn make_password_salt() -> String {
    SaltString::generate(&mut OsRng).to_string()
}

impl Db {
    pub fn get_web_accounts(&self) -> anyhow::Result<Vec<WebAccountRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, coalesce(login,''), coalesce(role,'viewer'), coalesce(enabled,true), kpz_from, kpz_to
                     from public.web_users
                     order by login, id",
                    &[],
                )
                .await?;
            Ok(rows
                .into_iter()
                .map(|r| WebAccountRow {
                    id: r.get::<_, i64>(0),
                    login: r.get::<_, String>(1),
                    password: String::new(),
                    role: r.get::<_, String>(2),
                    enabled: r.get::<_, bool>(3),
                    kpz_from: r.try_get::<_, Option<i32>>(4).ok().flatten(),
                    kpz_to: r.try_get::<_, Option<i32>>(5).ok().flatten(),
                })
                .collect())
        })
    }

    pub fn upsert_web_account(&self, row: &WebAccountRow) -> anyhow::Result<i64> {
        self.rt.block_on(async {
            let saved = if row.id > 0 {
                if row.password.trim().is_empty() {
                    self.client
                        .query_one(
                            "update public.web_users
                             set login = $2, role = $3, enabled = $4, kpz_from = $5, kpz_to = $6
                             where id = $1
                             returning id",
                            &[&row.id, &row.login, &row.role, &row.enabled, &row.kpz_from, &row.kpz_to],
                        )
                        .await?
                } else {
                    let salt = make_password_salt();
                    let hash = hash_web_password(&salt, &row.password)?;
                    self.client
                        .query_one(
                            "update public.web_users
                             set login = $2, password_salt = $3, password_hash = $4, role = $5, enabled = $6, kpz_from = $7, kpz_to = $8
                             where id = $1
                             returning id",
                            &[&row.id, &row.login, &salt, &hash, &row.role, &row.enabled, &row.kpz_from, &row.kpz_to],
                        )
                        .await?
                }
            } else {
                let salt = make_password_salt();
                let hash = hash_web_password(&salt, &row.password)?;
                self.client
                    .query_one(
                        "insert into public.web_users(login, password_salt, password_hash, role, enabled, kpz_from, kpz_to)
                         values($1,$2,$3,$4,$5,$6,$7)
                         returning id",
                        &[&row.login, &salt, &hash, &row.role, &row.enabled, &row.kpz_from, &row.kpz_to],
                    )
                    .await?
            };
            Ok(saved.get::<_, i64>(0))
        })
    }

    pub fn delete_web_account(&self, id: i64) -> anyhow::Result<()> {
        self.rt.block_on(async {
            self.client
                .execute("delete from public.web_users where id = $1", &[&id])
                .await?;
            Ok(())
        })
    }
}
