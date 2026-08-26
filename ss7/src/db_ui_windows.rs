use anyhow::Result;

use crate::db::Db;
use crate::models::{
    UiKpzTemplateLinkRow, UiKpzWindowRow, UiScreenTemplateRow, UiWindowBindingRow,
    UiWindowTextItemRow,
};

impl Db {
    pub fn get_ui_kpz_windows(&self, kpz_id: i32) -> Result<Vec<UiKpzWindowRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, kpz_id, code, title, description, is_active \
                     from ui.kpz_window \
                     where kpz_id = $1 \
                     order by code",
                    &[&kpz_id],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| UiKpzWindowRow {
                    id: r.get::<_, i64>(0),
                    kpz_id: r.get::<_, i32>(1),
                    code: r.get::<_, String>(2),
                    title: r.get::<_, String>(3),
                    description: r.try_get::<_, Option<String>>(4).ok().flatten(),
                    is_active: r.get::<_, bool>(5),
                })
                .collect();
            Ok(out)
        })
    }

    pub fn upsert_ui_kpz_window(
        &self,
        kpz_id: i32,
        code: &str,
        title: &str,
        description: Option<&str>,
        is_active: bool,
    ) -> Result<i64> {
        self.rt.block_on(async {
            let row = self
                .client
                .query_one(
                    "insert into ui.kpz_window(kpz_id, code, title, description, is_active) \
                     values($1, $2, $3, $4, $5) \
                     on conflict (kpz_id, code) do update set \
                       title = excluded.title, \
                       description = excluded.description, \
                       is_active = excluded.is_active, \
                       updated_at = now() \
                     returning id",
                    &[&kpz_id, &code, &title, &description, &is_active],
                )
                .await?;
            Ok(row.get::<_, i64>(0))
        })
    }

    pub fn is_ui_kpz_window_different_from_template(&self, window_id: i64, template_id: i64) -> Result<bool> {
        self.rt.block_on(async {
            let row = self
                .client
                .query_one(
                    "with w as ( \
                        select reg_id, pos, x, y, w, h, visible, writable, \
                               coalesce(label_override, '') as label_override, \
                               coalesce(unit, '') as unit, \
                               coalesce(fmt, '') as fmt, \
                               coalesce(web_safe_muted, false) as web_safe_muted \
                        from ui.kpz_window_reg_binding where window_id = $1 \
                     ), \
                     t as ( \
                        select reg_id, pos, x, y, w, h, visible, writable, \
                               coalesce(label_override, '') as label_override, \
                               coalesce(unit, '') as unit, \
                               coalesce(fmt, '') as fmt, \
                               coalesce(web_safe_muted, false) as web_safe_muted \
                        from ui.kpz_window_template_binding where template_id = $2 \
                     ) \
                     select exists( \
                        (select * from w except select * from t) \
                        union all \
                        (select * from t except select * from w) \
                     )",
                    &[&window_id, &template_id],
                )
                .await?;
            Ok(row.get::<_, bool>(0))
        })
    }

    pub fn delete_ui_kpz_window(&self, window_id: i64) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute("delete from ui.kpz_window where id = $1", &[&window_id])
                .await?;
            Ok(())
        })
    }

    pub fn delete_ui_kpz_windows_by_kpz(&self, kpz_id: i32) -> Result<u64> {
        self.rt.block_on(async {
            let n = self
                .client
                .execute("delete from ui.kpz_window where kpz_id = $1", &[&kpz_id])
                .await?;
            Ok(n)
        })
    }

    pub fn get_ui_kpz_window_templates(&self) -> Result<Vec<UiScreenTemplateRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select id, code, title, description, is_active \
                     from ui.kpz_window_template \
                     order by code",
                    &[],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| UiScreenTemplateRow {
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

    pub fn get_ui_kpz_template_links(&self, kpz_id: i32) -> Result<Vec<UiKpzTemplateLinkRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select l.template_id, t.code, t.title, t.description, l.is_default, l.sort_order \
                     from ui.kpz_template_link l \
                     join ui.kpz_window_template t on t.id = l.template_id \
                     where l.kpz_id = $1 \
                     order by l.sort_order, t.code",
                    &[&kpz_id],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| UiKpzTemplateLinkRow {
                    template_id: r.get::<_, i64>(0),
                    code: r.get::<_, String>(1),
                    title: r.get::<_, String>(2),
                    description: r.try_get::<_, Option<String>>(3).ok().flatten(),
                    is_default: r.get::<_, bool>(4),
                    sort_order: r.get::<_, i32>(5),
                })
                .collect();
            Ok(out)
        })
    }

    pub fn link_ui_template_to_kpz(&self, kpz_id: i32, template_id: i64, is_default: bool) -> Result<()> {
        self.rt.block_on(async {
            self.client.execute("begin", &[]).await?;
            if is_default {
                self.client
                    .execute(
                        "update ui.kpz_template_link set is_default = false, updated_at = now() where kpz_id = $1",
                        &[&kpz_id],
                    )
                    .await?;
            }
            self.client
                .execute(
                    "insert into ui.kpz_template_link(kpz_id, template_id, is_default, sort_order) \
                     values($1, $2, $3, coalesce((select max(sort_order) + 10 from ui.kpz_template_link where kpz_id = $1), 10)) \
                     on conflict (kpz_id, template_id) do update set \
                        is_default = excluded.is_default, \
                        updated_at = now()",
                    &[&kpz_id, &template_id, &is_default],
                )
                .await?;
            self.client.execute("commit", &[]).await?;
            Ok(())
        })
    }

    pub fn unlink_ui_template_from_kpz(&self, kpz_id: i32, template_id: i64) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute(
                    "delete from ui.kpz_template_link where kpz_id = $1 and template_id = $2",
                    &[&kpz_id, &template_id],
                )
                .await?;
            Ok(())
        })
    }

    pub fn upsert_ui_kpz_window_template(
        &self,
        template_id: Option<i64>,
        code: &str,
        title: &str,
        description: Option<&str>,
        is_active: bool,
    ) -> Result<i64> {
        self.rt.block_on(async {
            let row = if let Some(template_id) = template_id {
                self.client
                    .query_one(
                        "insert into ui.kpz_window_template(id, code, title, description, is_active) \
                         values($1, $2, $3, $4, $5) \
                         on conflict (id) do update set \
                            code = excluded.code, \
                            title = excluded.title, \
                            description = excluded.description, \
                            is_active = excluded.is_active, \
                            updated_at = now() \
                         returning id",
                        &[&template_id, &code, &title, &description, &is_active],
                    )
                    .await?
            } else {
                self.client
                    .query_one(
                        "insert into ui.kpz_window_template(code, title, description, is_active) \
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

    #[allow(dead_code)]
    pub fn delete_ui_kpz_window_template(&self, template_id: i64) -> Result<()> {
        self.rt.block_on(async {
            self.client
                .execute("delete from ui.kpz_window_template where id = $1", &[&template_id])
                .await?;
            Ok(())
        })
    }

    #[allow(dead_code)]
    pub fn upsert_ui_kpz_window_template_from_window(
        &self,
        window_id: i64,
        code: &str,
        title: &str,
        description: Option<&str>,
    ) -> Result<i64> {
        self.rt.block_on(async {
            self.client.execute("begin", &[]).await?;
            let tpl_id = match self
                .client
                .query_one(
                    "insert into ui.kpz_window_template(code, title, description, source_window_id, is_active) \
                     values($1, $2, $3, $4, true) \
                     on conflict (code) do update set \
                        title = excluded.title, \
                        description = excluded.description, \
                        source_window_id = excluded.source_window_id, \
                        is_active = true, \
                        updated_at = now() \
                     returning id",
                    &[&code, &title, &description, &window_id],
                )
                .await
            {
                Ok(row) => row.get::<_, i64>(0),
                Err(e) => {
                    let _ = self.client.execute("rollback", &[]).await;
                    return Err(e.into());
                }
            };

            if let Err(e) = self
                .client
                .execute(
                    "delete from ui.kpz_window_template_binding where template_id = $1",
                    &[&tpl_id],
                )
                .await
            {
                let _ = self.client.execute("rollback", &[]).await;
                return Err(e.into());
            }

            if let Err(e) = self
                .client
                .execute(
                    "insert into ui.kpz_window_template_binding( \
                        template_id, reg_id, pos, x, y, w, h, visible, writable, label_override, unit, fmt, scale_max, web_safe_muted \
                     ) \
                     select $1, reg_id, pos, x, y, w, h, visible, writable, label_override, unit, fmt, scale_max, web_safe_muted \
                     from ui.kpz_window_reg_binding \
                     where window_id = $2",
                    &[&tpl_id, &window_id],
                )
                .await
            {
                let _ = self.client.execute("rollback", &[]).await;
                return Err(e.into());
            }

            if let Err(e) = self
                .client
                .execute(
                    "delete from ui.kpz_window_template_text_item where template_id = $1",
                    &[&tpl_id],
                )
                .await
            {
                let _ = self.client.execute("rollback", &[]).await;
                return Err(e.into());
            }

            if let Err(e) = self
                .client
                .execute(
                    "insert into ui.kpz_window_template_text_item( \
                        template_id, pos, x, y, w, h, visible, text, item_kind, image_path, fit_mode, opacity, web_safe_muted \
                     ) \
                     select $1, pos, x, y, w, h, visible, text, item_kind, image_path, fit_mode, opacity, web_safe_muted \
                     from ui.kpz_window_text_item \
                     where window_id = $2",
                    &[&tpl_id, &window_id],
                )
                .await
            {
                let _ = self.client.execute("rollback", &[]).await;
                return Err(e.into());
            }

            self.client.execute("commit", &[]).await?;
            Ok(tpl_id)
        })
    }

    pub fn apply_ui_kpz_window_template_to_window(&self, template_id: i64, window_id: i64) -> Result<()> {
        self.rt.block_on(async {
            self.client.execute("begin", &[]).await?;

            if let Err(e) = self
                .client
                .execute("delete from ui.kpz_window_reg_binding where window_id = $1", &[&window_id])
                .await
            {
                let _ = self.client.execute("rollback", &[]).await;
                return Err(e.into());
            }

            if let Err(e) = self
                .client
                .execute(
                    "insert into ui.kpz_window_reg_binding( \
                        window_id, reg_id, pos, x, y, w, h, visible, writable, label_override, unit, fmt, scale_max, component_kind, web_safe_muted \
                     ) \
                     select $2, reg_id, pos, x, y, w, h, visible, writable, label_override, unit, fmt, scale_max, component_kind, web_safe_muted \
                     from ui.kpz_window_template_binding \
                     where template_id = $1",
                    &[&template_id, &window_id],
                )
                .await
            {
                let _ = self.client.execute("rollback", &[]).await;
                return Err(e.into());
            }

            if let Err(e) = self
                .client
                .execute("delete from ui.kpz_window_text_item where window_id = $1", &[&window_id])
                .await
            {
                let _ = self.client.execute("rollback", &[]).await;
                return Err(e.into());
            }

            if let Err(e) = self
                .client
                .execute(
                    "insert into ui.kpz_window_text_item( \
                        window_id, pos, x, y, w, h, visible, text, item_kind, image_path, fit_mode, opacity, web_safe_muted \
                     ) \
                     select $2, pos, x, y, w, h, visible, text, item_kind, image_path, fit_mode, opacity, web_safe_muted \
                     from ui.kpz_window_template_text_item \
                     where template_id = $1",
                    &[&template_id, &window_id],
                )
                .await
            {
                let _ = self.client.execute("rollback", &[]).await;
                return Err(e.into());
            }

            self.client.execute("commit", &[]).await?;
            Ok(())
        })
    }

    pub fn get_ui_template_bindings(&self, template_id: i64) -> Result<Vec<UiWindowBindingRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select b.reg_id, b.pos, coalesce(b.x,20), coalesce(b.y,20), \
                            coalesce(b.w,120), coalesce(b.h,34), \
                            b.visible, b.writable, b.label_override, b.unit, b.fmt, b.scale_max, b.component_kind, b.web_safe_muted, \
                            coalesce(r.name,''), coalesce(r.mb,0), coalesce(r.n_mb,0), coalesce(r.tip,0), r.bits \
                     from ui.kpz_window_template_binding b \
                     join public.reg r on r.id = b.reg_id \
                     where b.template_id = $1 \
                     order by b.pos, b.reg_id",
                    &[&template_id],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| UiWindowBindingRow {
                    reg_id: r.get::<_, i32>(0),
                    is_text: false,
                    pos: r.get::<_, i32>(1),
                    x: r.get::<_, i32>(2),
                    y: r.get::<_, i32>(3),
                    w: r.get::<_, i32>(4),
                    h: r.get::<_, i32>(5),
                    visible: r.get::<_, bool>(6),
                    writable: r.get::<_, bool>(7),
                    label_override: r.try_get::<_, Option<String>>(8).ok().flatten(),
                    unit: r.try_get::<_, Option<String>>(9).ok().flatten(),
                    fmt: r.try_get::<_, Option<String>>(10).ok().flatten(),
                    scale_max: r.try_get::<_, Option<f64>>(11).ok().flatten(),
                    component_kind: r.try_get::<_, Option<String>>(12).ok().flatten(),
                    web_safe_muted: r.get::<_, bool>(13),
                    reg_name: r.get::<_, String>(14),
                    reg_mb: r.get::<_, i32>(15),
                    reg_n_mb: r.get::<_, i32>(16),
                    reg_tip: r.get::<_, i32>(17),
                    reg_bits: r.try_get::<_, Option<i32>>(18).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }

    pub fn save_ui_template_bindings(&self, template_id: i64, bindings: &[UiWindowBindingRow]) -> Result<()> {
        self.rt.block_on(async {
            self.client.execute("begin", &[]).await?;
            self.client
                .execute(
                    "delete from ui.kpz_window_template_binding where template_id = $1",
                    &[&template_id],
                )
                .await?;

            for b in bindings {
                if let Err(e) = self
                    .client
                    .execute(
                        "insert into ui.kpz_window_template_binding(\
                            template_id, reg_id, pos, x, y, w, h, visible, writable, label_override, unit, fmt, scale_max, component_kind, web_safe_muted\
                         ) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
                        &[
                            &template_id,
                            &b.reg_id,
                            &b.pos,
                            &b.x,
                            &b.y,
                            &b.w,
                            &b.h,
                            &b.visible,
                            &b.writable,
                            &b.label_override,
                            &b.unit,
                            &b.fmt,
                            &b.scale_max,
                            &b.component_kind,
                            &b.web_safe_muted,
                        ],
                    )
                    .await
                {
                    let _ = self.client.execute("rollback", &[]).await;
                    return Err(e.into());
                }
            }
            self.client.execute("commit", &[]).await?;
            Ok(())
        })
    }

    pub fn get_ui_template_text_items(&self, template_id: i64) -> Result<Vec<UiWindowTextItemRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select pos, coalesce(x,20), coalesce(y,20), \
                            coalesce(w,120), coalesce(h,34), visible, coalesce(text,''), \
                            coalesce(item_kind,'text'), image_path, coalesce(fit_mode,'contain'), \
                            coalesce(opacity,1.0), coalesce(web_safe_muted, false) \
                     from ui.kpz_window_template_text_item \
                     where template_id = $1 \
                     order by pos, id",
                    &[&template_id],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| UiWindowTextItemRow {
                    pos: r.get::<_, i32>(0),
                    x: r.get::<_, i32>(1),
                    y: r.get::<_, i32>(2),
                    w: r.get::<_, i32>(3),
                    h: r.get::<_, i32>(4),
                    visible: r.get::<_, bool>(5),
                    text: r.get::<_, String>(6),
                    item_kind: r.get::<_, String>(7),
                    image_path: r.try_get::<_, Option<String>>(8).ok().flatten(),
                    fit_mode: r.get::<_, String>(9),
                    opacity: r.get::<_, f64>(10),
                    web_safe_muted: r.get::<_, bool>(11),
                })
                .collect();
            Ok(out)
        })
    }

    pub fn save_ui_template_text_items(&self, template_id: i64, items: &[UiWindowTextItemRow]) -> Result<()> {
        self.rt.block_on(async {
            self.client.execute("begin", &[]).await?;
            self.client
                .execute(
                    "delete from ui.kpz_window_template_text_item where template_id = $1",
                    &[&template_id],
                )
                .await?;

            for it in items {
                if let Err(e) = self
                    .client
                    .execute(
                        "insert into ui.kpz_window_template_text_item(\
                            template_id, pos, x, y, w, h, visible, text, item_kind, image_path, fit_mode, opacity, web_safe_muted\
                         ) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
                        &[
                            &template_id,
                            &it.pos,
                            &it.x,
                            &it.y,
                            &it.w,
                            &it.h,
                            &it.visible,
                            &it.text,
                            &it.item_kind,
                            &it.image_path,
                            &it.fit_mode,
                            &it.opacity,
                            &it.web_safe_muted,
                        ],
                    )
                    .await
                {
                    let _ = self.client.execute("rollback", &[]).await;
                    return Err(e.into());
                }
            }
            self.client.execute("commit", &[]).await?;
            Ok(())
        })
    }

    pub fn get_ui_window_bindings(&self, window_id: i64) -> Result<Vec<UiWindowBindingRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select b.reg_id, b.pos, coalesce(b.x,20), coalesce(b.y,20), \
                            coalesce(b.w,120), coalesce(b.h,34), \
                            b.visible, b.writable, b.label_override, b.unit, b.fmt, b.scale_max, b.component_kind, b.web_safe_muted, \
                            coalesce(r.name,''), coalesce(r.mb,0), coalesce(r.n_mb,0), coalesce(r.tip,0), r.bits \
                     from ui.kpz_window_reg_binding b \
                     join public.reg r on r.id = b.reg_id \
                     where b.window_id = $1 \
                     order by b.pos, b.reg_id",
                    &[&window_id],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| UiWindowBindingRow {
                    reg_id: r.get::<_, i32>(0),
                    is_text: false,
                    pos: r.get::<_, i32>(1),
                    x: r.get::<_, i32>(2),
                    y: r.get::<_, i32>(3),
                    w: r.get::<_, i32>(4),
                    h: r.get::<_, i32>(5),
                    visible: r.get::<_, bool>(6),
                    writable: r.get::<_, bool>(7),
                    label_override: r.try_get::<_, Option<String>>(8).ok().flatten(),
                    unit: r.try_get::<_, Option<String>>(9).ok().flatten(),
                    fmt: r.try_get::<_, Option<String>>(10).ok().flatten(),
                    scale_max: r.try_get::<_, Option<f64>>(11).ok().flatten(),
                    component_kind: r.try_get::<_, Option<String>>(12).ok().flatten(),
                    web_safe_muted: r.get::<_, bool>(13),
                    reg_name: r.get::<_, String>(14),
                    reg_mb: r.get::<_, i32>(15),
                    reg_n_mb: r.get::<_, i32>(16),
                    reg_tip: r.get::<_, i32>(17),
                    reg_bits: r.try_get::<_, Option<i32>>(18).ok().flatten(),
                })
                .collect();
            Ok(out)
        })
    }

    pub fn get_ui_window_text_items(&self, window_id: i64) -> Result<Vec<UiWindowTextItemRow>> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "select pos, coalesce(x,20), coalesce(y,20), \
                            coalesce(w,120), coalesce(h,34), visible, coalesce(text,''), \
                            coalesce(item_kind,'text'), image_path, coalesce(fit_mode,'contain'), \
                            coalesce(opacity,1.0), coalesce(web_safe_muted, false) \
                     from ui.kpz_window_text_item \
                     where window_id = $1 \
                     order by pos, id",
                    &[&window_id],
                )
                .await?;
            let out = rows
                .into_iter()
                .map(|r| UiWindowTextItemRow {
                    pos: r.get::<_, i32>(0),
                    x: r.get::<_, i32>(1),
                    y: r.get::<_, i32>(2),
                    w: r.get::<_, i32>(3),
                    h: r.get::<_, i32>(4),
                    visible: r.get::<_, bool>(5),
                    text: r.get::<_, String>(6),
                    item_kind: r.get::<_, String>(7),
                    image_path: r.try_get::<_, Option<String>>(8).ok().flatten(),
                    fit_mode: r.get::<_, String>(9),
                    opacity: r.get::<_, f64>(10),
                    web_safe_muted: r.get::<_, bool>(11),
                })
                .collect();
            Ok(out)
        })
    }

    pub fn save_ui_window_bindings(&self, window_id: i64, bindings: &[UiWindowBindingRow]) -> Result<()> {
        self.rt.block_on(async {
            self.client.execute("begin", &[]).await?;
            self.client
                .execute(
                    "delete from ui.kpz_window_reg_binding where window_id = $1",
                    &[&window_id],
                )
                .await?;

            for b in bindings {
                if let Err(e) = self
                    .client
                    .execute(
                        "insert into ui.kpz_window_reg_binding(\
                        window_id, reg_id, pos, x, y, w, h, visible, writable, label_override, unit, fmt, scale_max, component_kind, web_safe_muted\
                     ) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
                        &[
                            &window_id,
                            &b.reg_id,
                            &b.pos,
                            &b.x,
                            &b.y,
                            &b.w,
                            &b.h,
                            &b.visible,
                            &b.writable,
                            &b.label_override,
                            &b.unit,
                            &b.fmt,
                            &b.scale_max,
                            &b.component_kind,
                            &b.web_safe_muted,
                        ],
                    )
                    .await
                {
                    let _ = self.client.execute("rollback", &[]).await;
                    return Err(e.into());
                }
            }
            self.client.execute("commit", &[]).await?;
            Ok(())
        })
    }

    pub fn save_ui_window_text_items(&self, window_id: i64, items: &[UiWindowTextItemRow]) -> Result<()> {
        self.rt.block_on(async {
            self.client.execute("begin", &[]).await?;
            self.client
                .execute(
                    "delete from ui.kpz_window_text_item where window_id = $1",
                    &[&window_id],
                )
                .await?;

            for it in items {
                if let Err(e) = self
                    .client
                    .execute(
                        "insert into ui.kpz_window_text_item(\
                            window_id, pos, x, y, w, h, visible, text, item_kind, image_path, fit_mode, opacity, web_safe_muted\
                         ) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
                        &[
                            &window_id,
                            &it.pos,
                            &it.x,
                            &it.y,
                            &it.w,
                            &it.h,
                            &it.visible,
                            &it.text,
                            &it.item_kind,
                            &it.image_path,
                            &it.fit_mode,
                            &it.opacity,
                            &it.web_safe_muted,
                        ],
                    )
                    .await
                {
                    let _ = self.client.execute("rollback", &[]).await;
                    return Err(e.into());
                }
            }
            self.client.execute("commit", &[]).await?;
            Ok(())
        })
    }

    pub fn sync_template_images_to_matching_windows(&self, template_id: i64) -> Result<(usize, usize)> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "insert into ui.kpz_window_text_item(\
                        window_id, pos, x, y, w, h, visible, text, item_kind, image_path, fit_mode, opacity, web_safe_muted\
                     ) \
                     select w.id, t.pos, t.x, t.y, t.w, t.h, t.visible, t.text, t.item_kind, \
                            t.image_path, t.fit_mode, t.opacity, t.web_safe_muted \
                     from ui.kpz_window_template tpl \
                     join ui.kpz_window w on w.code = tpl.code and w.is_active \
                     join ui.kpz_window_template_text_item t on t.template_id = tpl.id \
                     where tpl.id = $1 \
                       and coalesce(t.item_kind,'text') = 'image' \
                       and not exists ( \
                           select 1 \
                           from ui.kpz_window_text_item wt \
                           where wt.window_id = w.id \
                             and coalesce(wt.item_kind,'text') = 'image' \
                             and (wt.pos = t.pos or coalesce(wt.image_path, wt.text, '') = coalesce(t.image_path, t.text, '')) \
                       ) \
                     on conflict (window_id, pos) do nothing \
                     returning window_id",
                    &[&template_id],
                )
                .await?;
            let mut windows = std::collections::BTreeSet::new();
            for r in &rows {
                windows.insert(r.get::<_, i64>(0));
            }
            Ok((rows.len(), windows.len()))
        })
    }

    pub fn update_template_images_in_matching_windows(&self, template_id: i64) -> Result<(usize, usize)> {
        self.rt.block_on(async {
            let rows = self
                .client
                .query(
                    "update ui.kpz_window_text_item wt \
                     set x = t.x, y = t.y, w = t.w, h = t.h, visible = t.visible, text = t.text, \
                         image_path = t.image_path, fit_mode = t.fit_mode, opacity = t.opacity, \
                         web_safe_muted = t.web_safe_muted, updated_at = now() \
                     from ui.kpz_window_template tpl \
                     join ui.kpz_window w on w.code = tpl.code and w.is_active \
                     join ui.kpz_window_template_text_item t on t.template_id = tpl.id \
                     where tpl.id = $1 \
                       and wt.window_id = w.id \
                       and coalesce(wt.item_kind,'text') = 'image' \
                       and coalesce(t.item_kind,'text') = 'image' \
                       and (wt.pos = t.pos or coalesce(wt.image_path, wt.text, '') = coalesce(t.image_path, t.text, '')) \
                     returning wt.window_id",
                    &[&template_id],
                )
                .await?;
            let mut windows = std::collections::BTreeSet::new();
            for r in &rows {
                windows.insert(r.get::<_, i64>(0));
            }
            Ok((rows.len(), windows.len()))
        })
    }
}
