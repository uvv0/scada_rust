use anyhow::Result;

use crate::db::Db;
use crate::models::{UiKpTemplateRow, UiKpTemplateWindowRow, UiKpzKpTemplateLinkRow};

impl Db {
    pub fn get_ui_kp_templates(&self) -> Result<Vec<UiKpTemplateRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, code, title, description, is_active \
                     from ui.kp_template \
                     order by code",
                    &[],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| UiKpTemplateRow {
                    id: r.get::<_, i64>(0),
                    code: r.get::<_, String>(1),
                    title: r.get::<_, String>(2),
                    description: r.try_get::<_, Option<String>>(3).ok().flatten(),
                    is_active: r.get::<_, bool>(4),
                })
                .collect();
            Ok(out)
        })
    }

    pub fn upsert_ui_kp_template(
        &self,
        kp_template_id: Option<i64>,
        code: &str,
        title: &str,
        description: Option<&str>,
        is_active: bool,
    ) -> Result<i64> {
        self.rt.block_on(async {
            let row = if let Some(id) = kp_template_id {
                self.client
                    .query_one(
                        "insert into ui.kp_template(id, code, title, description, is_active) \
                         values($1, $2, $3, $4, $5) \
                         on conflict (id) do update set \
                            code = excluded.code, \
                            title = excluded.title, \
                            description = excluded.description, \
                            is_active = excluded.is_active, \
                            updated_at = now() \
                         returning id",
                        &[&id, &code, &title, &description, &is_active],
                    )
                    .await?
            } else {
                self.client
                    .query_one(
                        "insert into ui.kp_template(code, title, description, is_active) \
                         values($1, $2, $3, $4) \
                         on conflict (code) do update set \
                            title = excluded.title, \
                            description = excluded.description, \
                            is_active = excluded.is_active, \
                            updated_at = now() \
                         returning id",
                        &[&code, &title, &description, &is_active],
                    )
                    .await?
            };
            Ok(row.get::<_, i64>(0))
        })
    }

    pub fn delete_ui_kp_template(&self, kp_template_id: i64) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute("delete from ui.kp_template where id = $1", &[&kp_template_id])
                .await?;
            Ok(())
        })
    }

    pub fn get_ui_kp_template_windows(&self, kp_template_id: i64) -> Result<Vec<UiKpTemplateWindowRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select w.kp_template_id, w.window_template_id, t.code, t.title, w.sort_order, w.is_default \
                     from ui.kp_template_window w \
                     join ui.kpz_window_template t on t.id = w.window_template_id \
                     where w.kp_template_id = $1 \
                     order by w.sort_order, t.code",
                    &[&kp_template_id],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| UiKpTemplateWindowRow {
                    kp_template_id: r.get::<_, i64>(0),
                    window_template_id: r.get::<_, i64>(1),
                    window_template_code: r.get::<_, String>(2),
                    window_template_title: r.get::<_, String>(3),
                    sort_order: r.get::<_, i32>(4),
                    is_default: r.get::<_, bool>(5),
                })
                .collect();
            Ok(out)
        })
    }

    pub fn add_ui_window_template_to_kp_template(
        &self,
        kp_template_id: i64,
        window_template_id: i64,
    ) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute(
                    "insert into ui.kp_template_window(kp_template_id, window_template_id, sort_order) \
                     values( \
                        $1, \
                        $2, \
                        coalesce((select max(sort_order) + 10 from ui.kp_template_window where kp_template_id = $1), 10) \
                     ) \
                     on conflict (kp_template_id, window_template_id) do nothing",
                    &[&kp_template_id, &window_template_id],
                )
                .await?;
            Ok(())
        })
    }

    pub fn remove_ui_window_template_from_kp_template(
        &self,
        kp_template_id: i64,
        window_template_id: i64,
    ) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute(
                    "delete from ui.kp_template_window \
                     where kp_template_id = $1 and window_template_id = $2",
                    &[&kp_template_id, &window_template_id],
                )
                .await?;
            Ok(())
        })
    }

    pub fn get_ui_kpz_kp_template_link(&self, kpz_id: i32) -> Result<Option<UiKpzKpTemplateLinkRow>> {
        self.rt.block_on(async {
            let row = self
                .client
                .query_opt(
                    "select l.kpz_id, l.kp_template_id, t.code, t.title \
                     from ui.kpz_kp_template_link l \
                     join ui.kp_template t on t.id = l.kp_template_id \
                     where l.kpz_id = $1",
                    &[&kpz_id],
                )
                .await?;
            Ok(row.map(|r| UiKpzKpTemplateLinkRow {
                kpz_id: r.get::<_, i32>(0),
                kp_template_id: r.get::<_, i64>(1),
                kp_template_code: r.get::<_, String>(2),
                kp_template_title: r.get::<_, String>(3),
            }))
        })
    }

    pub fn set_ui_kpz_kp_template_link(&self, kpz_id: i32, kp_template_id: i64) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute(
                    "insert into ui.kpz_kp_template_link(kpz_id, kp_template_id) \
                     values($1, $2) \
                     on conflict (kpz_id) do update set \
                        kp_template_id = excluded.kp_template_id, \
                        updated_at = now()",
                    &[&kpz_id, &kp_template_id],
                )
                .await?;
            Ok(())
        })
    }

    pub fn clear_ui_kpz_kp_template_link(&self, kpz_id: i32) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute("delete from ui.kpz_kp_template_link where kpz_id = $1", &[&kpz_id])
                .await?;
            Ok(())
        })
    }
}
