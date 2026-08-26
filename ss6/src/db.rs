use anyhow::Result;
use std::collections::HashMap;
use tokio_postgres::Client;

use crate::models::{
    AlarmRulePreviewDto, ArxPointDto, ArxSeriesDto, GroupDto, IoConnDto, KpzDto, LiveValueDto,
    RegDto, RegIoDto, UiBindingDto, UiWindowDto, WebActionDto, WebSessionUserDto, WebUserAuthRow,
};

pub async fn ensure_web_auth_schema(client: &Client) -> Result<()> {
    client
        .batch_execute(
            "create table if not exists web_users (
                id bigserial primary key,
                login text not null unique,
                password_salt text not null,
                password_hash text not null,
                role text not null default 'admin',
                enabled boolean not null default true,
                kpz_from int,
                kpz_to int,
                created_at timestamptz not null default now()
            );
            create table if not exists web_sessions (
                id bigserial primary key,
                user_id bigint not null references web_users(id) on delete cascade,
                session_token text not null unique,
                created_at timestamptz not null default now(),
                expires_at timestamptz not null
            );
            create index if not exists web_sessions_token_idx on web_sessions(session_token);
            create index if not exists web_sessions_user_idx on web_sessions(user_id);
            alter table if exists web_users add column if not exists kpz_from int;
            alter table if exists web_users add column if not exists kpz_to int;
            create table if not exists web_actions (
                id bigserial primary key,
                user_id bigint not null references web_users(id) on delete cascade,
                action text not null,
                detail text not null default '',
                kpz_id int,
                reg_id int,
                created_at timestamptz not null default now()
            );
            create index if not exists web_actions_created_idx on web_actions(created_at desc);",
        )
        .await?;
    Ok(())
}

pub async fn count_web_users(client: &Client) -> Result<i64> {
    let row = client.query_one("select count(*) from web_users", &[]).await?;
    Ok(row.get::<_, i64>(0))
}

pub async fn insert_web_user(
    client: &Client,
    login: &str,
    password_salt: &str,
    password_hash: &str,
    role: &str,
) -> Result<i64> {
    let row = client
        .query_one(
            "insert into web_users(login, password_salt, password_hash, role, enabled)
             values($1, $2, $3, $4, true)
             returning id",
            &[&login, &password_salt, &password_hash, &role],
        )
        .await?;
    Ok(row.get::<_, i64>(0))
}

pub async fn load_web_user_by_login(client: &Client, login: &str) -> Result<Option<WebUserAuthRow>> {
    let row = client
        .query_opt(
            "select id, login, password_salt, password_hash, role, enabled
             , kpz_from, kpz_to
             from web_users
             where lower(login) = lower($1)",
            &[&login],
        )
        .await?;
    Ok(row.map(|r| WebUserAuthRow {
        id: r.get::<_, i64>(0),
        login: r.get::<_, String>(1),
        password_salt: r.get::<_, String>(2),
        password_hash: r.get::<_, String>(3),
        role: r.get::<_, String>(4),
        enabled: r.get::<_, bool>(5),
        kpz_from: r.try_get::<_, Option<i32>>(6).ok().flatten(),
        kpz_to: r.try_get::<_, Option<i32>>(7).ok().flatten(),
    }))
}

pub async fn create_web_session(client: &Client, user_id: i64, session_token: &str) -> Result<()> {
    client
        .execute(
            "insert into web_sessions(user_id, session_token, expires_at)
             values($1, $2, now() + interval '30 days')",
            &[&user_id, &session_token],
        )
        .await?;
    Ok(())
}

pub async fn insert_web_action(
    client: &Client,
    user_id: i64,
    action: &str,
    detail: &str,
    kpz_id: Option<i32>,
    reg_id: Option<i32>,
) -> Result<()> {
    client
        .execute(
            "insert into web_actions(user_id, action, detail, kpz_id, reg_id) values($1, $2, $3, $4, $5)",
            &[&user_id, &action, &detail, &kpz_id, &reg_id],
        )
        .await?;
    Ok(())
}

pub async fn load_recent_web_actions(client: &Client, limit: i64) -> Result<Vec<WebActionDto>> {
    let rows = client
        .query(
            "select w.id, w.user_id, u.login, w.action, w.detail, w.kpz_id, w.reg_id,
                    to_char(w.created_at at time zone 'utc', 'YYYY-MM-DD HH24:MI:SS')
             from web_actions w join web_users u on u.id = w.user_id
             order by w.created_at desc limit $1",
            &[&limit],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| WebActionDto {
            id: r.get::<_, i64>(0),
            user_id: r.get::<_, i64>(1),
            login: r.get::<_, String>(2),
            action: r.get::<_, String>(3),
            detail: r.get::<_, String>(4),
            kpz_id: r.get::<_, Option<i32>>(5),
            reg_id: r.get::<_, Option<i32>>(6),
            created_at: r.get::<_, String>(7),
        })
        .collect())
}

pub async fn load_web_session_user(client: &Client, session_token: &str) -> Result<Option<WebSessionUserDto>> {
    let row = client
        .query_opt(
            "select u.id, u.login, u.role, u.kpz_from, u.kpz_to
             from web_sessions s
             join web_users u on u.id = s.user_id
             where s.session_token = $1
               and s.expires_at > now()
               and u.enabled = true",
            &[&session_token],
        )
        .await?;
    Ok(row.map(|r| WebSessionUserDto {
        user_id: r.get::<_, i64>(0),
        login: r.get::<_, String>(1),
        role: r.get::<_, String>(2),
        kpz_from: r.try_get::<_, Option<i32>>(3).ok().flatten(),
        kpz_to: r.try_get::<_, Option<i32>>(4).ok().flatten(),
    }))
}

pub async fn delete_web_session(client: &Client, session_token: &str) -> Result<()> {
    client
        .execute("delete from web_sessions where session_token = $1", &[&session_token])
        .await?;
    Ok(())
}

pub async fn load_kpz(client: &Client) -> Result<Vec<KpzDto>> {
    let rows = client
        .query(
            "select id, coalesce(name,''), coalesce(start,0) from kpz order by id",
            &[],
        )
        .await?;
    let out = rows
        .into_iter()
        .map(|r| KpzDto {
            id: r.get::<_, i32>(0),
            name: r.get::<_, String>(1),
            start: r.get::<_, i32>(2),
        })
        .collect();
    Ok(out)
}

pub async fn load_groups_for_kpz(client: &Client, kpz_id: i32) -> Result<Vec<GroupDto>> {
    let row = client
        .query_opt("select grups from kpz where id=$1", &[&kpz_id])
        .await?;
    let Some(row) = row else {
        return Ok(Vec::new());
    };
    let bytes: Vec<u8> = row.get::<_, Vec<u8>>(0);
    let ids = decode_groups(&bytes);
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = client
        .query(
            "select id, coalesce(name,'') from grup where id = any($1) order by id",
            &[&ids],
        )
        .await;

    match rows {
        Ok(rows) => {
            let mut out = rows
                .into_iter()
                .map(|r| GroupDto {
                    id: r.get::<_, i32>(0),
                    name: r.get::<_, String>(1),
                })
                .collect::<Vec<_>>();
            if out.is_empty() {
                out = ids
                    .into_iter()
                    .map(|id| GroupDto {
                        id,
                        name: format!("group {}", id),
                    })
                    .collect();
            }
            Ok(out)
        }
        Err(_) => Ok(ids
            .into_iter()
            .map(|id| GroupDto {
                id,
                name: format!("group {}", id),
            })
            .collect()),
    }
}

pub async fn load_regs_for_group(client: &Client, group_id: i32) -> Result<Vec<RegDto>> {
    let rows = client
        .query(
            "select id, coalesce(name,''), coalesce(mb,0), coalesce(tip,0), bits \
             from reg where grup = $1 order by mb asc nulls last, id asc",
            &[&group_id],
        )
        .await?;
    let out = rows
        .into_iter()
        .map(|r| RegDto {
            id: r.get::<_, i32>(0),
            name: r.get::<_, String>(1),
            mb: r.get::<_, i32>(2),
            tip: r.get::<_, i32>(3),
            bits: r.try_get::<_, Option<i32>>(4).ok().flatten(),
        })
        .collect();
    Ok(out)
}

pub async fn load_arx_series(
    client: &Client,
    kpz_id: i32,
    reg_ids: &[i32],
    limit: i64,
    window_sec: i64,
) -> Result<Vec<ArxSeriesDto>> {
    let rows = client
        .query(
            "select reg_id, ts_unix, val_num \
             from ( \
                select reg_id, ts_unix, val_num, \
                       row_number() over (partition by reg_id order by ts_unix desc) as rn \
                from arx_val \
                where kpz_id = $1 and reg_id = any($2) and val_num is not null \
                  and ts_unix >= (extract(epoch from now())::bigint - $4) \
             ) s \
             where rn <= $3 \
             order by reg_id, ts_unix asc",
            &[&kpz_id, &reg_ids, &limit, &window_sec],
        )
        .await?;

    let mut by_reg: std::collections::BTreeMap<i32, Vec<ArxPointDto>> =
        std::collections::BTreeMap::new();
    for r in rows {
        let reg_id = r.get::<_, i32>(0);
        let ts_unix = r.get::<_, i64>(1);
        let val_num = r.get::<_, f64>(2);
        by_reg
            .entry(reg_id)
            .or_default()
            .push(ArxPointDto { ts_unix, val_num });
    }

    let mut out = Vec::new();
    for (reg_id, points) in by_reg {
        out.push(ArxSeriesDto { reg_id, points });
    }
    Ok(out)
}

pub fn parse_reg_ids(s: &str) -> Vec<i32> {
    let mut out = Vec::new();
    for part in s.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if let Ok(v) = p.parse::<i32>() {
            if v > 0 {
                out.push(v);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Список окон для веб-просмотра: реальные окна из ui.kpz_window (ss7) + шаблоны из КП-шаблона (fallback).
pub async fn load_ui_windows_for_kpz(client: &Client, kpz_id: i32) -> Result<Vec<UiWindowDto>> {
    let mut out = Vec::new();

    // Реальные окна КПЗ (созданные в ss7), id > 0
    if let Ok(rows) = client
        .query(
            "select id, coalesce(code,''), coalesce(title,'') \
             from ui.kpz_window \
             where kpz_id = $1 and is_active = true \
             order by code",
            &[&kpz_id],
        )
        .await
    {
        for r in rows {
            out.push(UiWindowDto {
                id: r.get::<_, i64>(0),
                code: r.get::<_, String>(1),
                title: r.get::<_, String>(2),
                is_template: false,
            });
        }
    }

    // Шаблоны из привязки КП (если нет реальных окон или для совместимости), id < 0
    if let Ok(rows) = client
        .query(
            "select distinct on (t.id) -t.id as id, coalesce(t.code,''), coalesce(t.title,'') \
             from ui.kpz_kp_template_link l \
             join ui.kp_template_window w on w.kp_template_id = l.kp_template_id \
             join ui.kpz_window_template t on t.id = w.window_template_id \
             where l.kpz_id = $1 \
             order by t.id, w.sort_order, t.code",
            &[&kpz_id],
        )
        .await
    {
        for r in rows {
            out.push(UiWindowDto {
                id: r.get::<_, i64>(0),
                code: r.get::<_, String>(1),
                title: r.get::<_, String>(2),
                is_template: true,
            });
        }
    }

    out.sort_by(|a, b| a.code.cmp(&b.code).then(a.title.cmp(&b.title)).then(a.id.cmp(&b.id)));
    Ok(out)
}

pub async fn load_ui_bindings(client: &Client, window_id: i64) -> Result<Vec<UiBindingDto>> {
    if window_id < 0 {
        let template_id = -window_id;
        let rows = match client
            .query(
                "select b.reg_id, false as is_text, coalesce(b.x,20), coalesce(b.y,20), \
                        coalesce(b.w,120), coalesce(b.h,34), \
                        b.visible, b.writable, coalesce(r.name,''), \
                        coalesce(r.mb,0), coalesce(r.n_mb,0), coalesce(r.tip,0), r.bits, \
                        b.label_override, b.unit, b.fmt, b.scale_max, b.component_kind, b.pos \
                 from ui.kpz_window_template_binding b \
                 join public.reg r on r.id = b.reg_id \
                 where b.template_id = $1 \
                 union all \
                 select -1 * (1000000 + t.pos) as reg_id, true as is_text, \
                        coalesce(t.x,20), coalesce(t.y,20), coalesce(t.w,120), coalesce(t.h,34), \
                        t.visible, false as writable, \
                        case when coalesce(t.item_kind,'text') = 'image' then 'Image' else coalesce(t.text,'') end, \
                        0 as reg_mb, 0 as reg_n_mb, 0 as reg_tip, null::int as reg_bits, \
                        case when coalesce(t.item_kind,'text') = 'image' then coalesce(t.image_path, t.text, '') else null::text end as label_override, \
                        null::text as unit, \
                        case when coalesce(t.item_kind,'text') = 'image' then coalesce(t.fit_mode,'contain') else null::text end as fmt, \
                        case when coalesce(t.item_kind,'text') = 'image' then coalesce(t.opacity,1.0) else null::double precision end as scale_max, \
                        case when coalesce(t.item_kind,'text') = 'image' then 'image'::text else null::text end as component_kind, t.pos \
                 from ui.kpz_window_template_text_item t \
                 where t.template_id = $1 \
                 order by 19, 1",
                &[&template_id],
            )
            .await
        {
            Ok(v) => v,
            Err(_) => {
                client
                    .query(
                        "select b.reg_id, false as is_text, coalesce(b.x,20), coalesce(b.y,20), \
                                coalesce(b.w,120), coalesce(b.h,34), \
                                b.visible, b.writable, coalesce(r.name,''), \
                                coalesce(r.mb,0), coalesce(r.n_mb,0), coalesce(r.tip,0), r.bits, \
                                b.label_override, b.unit, b.fmt, b.scale_max, b.component_kind, b.pos \
                         from ui.kpz_window_template_binding b \
                         join public.reg r on r.id = b.reg_id \
                         where b.template_id = $1 \
                         order by b.pos, b.reg_id",
                        &[&template_id],
                    )
                    .await?
            }
        };
        let out = rows
            .into_iter()
            .map(|r| UiBindingDto {
                reg_id: r.get::<_, i32>(0),
                is_text: r.get::<_, bool>(1),
                x: r.get::<_, i32>(2),
                y: r.get::<_, i32>(3),
                w: r.get::<_, i32>(4),
                h: r.get::<_, i32>(5),
                visible: r.get::<_, bool>(6),
                writable: r.get::<_, bool>(7),
                reg_name: r.get::<_, String>(8),
                reg_mb: r.get::<_, i32>(9),
                reg_n_mb: r.get::<_, i32>(10),
                reg_tip: r.get::<_, i32>(11),
                reg_bits: r.try_get::<_, Option<i32>>(12).ok().flatten(),
                label_override: r.try_get::<_, Option<String>>(13).ok().flatten(),
                unit: r.try_get::<_, Option<String>>(14).ok().flatten(),
                fmt: r.try_get::<_, Option<String>>(15).ok().flatten(),
                scale_max: r.try_get::<_, Option<f64>>(16).ok().flatten(),
                component_kind: r.try_get::<_, Option<String>>(17).ok().flatten(),
            })
            .collect();
        return Ok(out);
    }
    let rows = match client
        .query(
            "select b.reg_id, false as is_text, coalesce(b.x,20), coalesce(b.y,20), \
                    coalesce(b.w,120), coalesce(b.h,34), \
                    b.visible, b.writable, coalesce(r.name,''), \
                    coalesce(r.mb,0), coalesce(r.n_mb,0), coalesce(r.tip,0), r.bits, \
                    b.label_override, b.unit, b.fmt, b.scale_max, b.component_kind, b.pos \
             from ui.kpz_window_reg_binding b \
             join public.reg r on r.id = b.reg_id \
             where b.window_id = $1 \
             union all \
             select -1 * (1000000 + t.pos) as reg_id, true as is_text, \
                     coalesce(t.x,20), coalesce(t.y,20), coalesce(t.w,120), coalesce(t.h,34), \
                     t.visible, false as writable, \
                     case when coalesce(t.item_kind,'text') = 'image' then 'Image' else coalesce(t.text,'') end, \
                     0 as reg_mb, 0 as reg_n_mb, 0 as reg_tip, null::int as reg_bits, \
                     case when coalesce(t.item_kind,'text') = 'image' then coalesce(t.image_path, t.text, '') else null::text end as label_override, \
                     null::text as unit, \
                     case when coalesce(t.item_kind,'text') = 'image' then coalesce(t.fit_mode,'contain') else null::text end as fmt, \
                     case when coalesce(t.item_kind,'text') = 'image' then coalesce(t.opacity,1.0) else null::double precision end as scale_max, \
                     case when coalesce(t.item_kind,'text') = 'image' then 'image'::text else null::text end as component_kind, t.pos \
             from ui.kpz_window_text_item t \
             where t.window_id = $1 \
             union all \
             select -1 * (1000000 + t.pos) as reg_id, true as is_text, \
                    coalesce(t.x,20), coalesce(t.y,20), coalesce(t.w,120), coalesce(t.h,34), \
                    t.visible, false as writable, \
                    case when coalesce(t.item_kind,'text') = 'image' then 'Image' else coalesce(t.text,'') end, \
                    0 as reg_mb, 0 as reg_n_mb, 0 as reg_tip, null::int as reg_bits, \
                    case when coalesce(t.item_kind,'text') = 'image' then coalesce(t.image_path, t.text, '') else null::text end as label_override, \
                    null::text as unit, \
                    case when coalesce(t.item_kind,'text') = 'image' then coalesce(t.fit_mode,'contain') else null::text end as fmt, \
                    case when coalesce(t.item_kind,'text') = 'image' then coalesce(t.opacity,1.0) else null::double precision end as scale_max, \
                    case when coalesce(t.item_kind,'text') = 'image' then 'image'::text else null::text end as component_kind, t.pos \
             from ui.kpz_window w \
             join ui.kpz_window_template tpl on tpl.code = w.code and tpl.is_active \
             join ui.kpz_window_template_text_item t on t.template_id = tpl.id \
             where w.id = $1 \
               and coalesce(t.item_kind,'text') = 'image' \
               and not exists ( \
                   select 1 \
                   from ui.kpz_window_text_item wt \
                   where wt.window_id = w.id \
                     and coalesce(wt.item_kind,'text') = 'image' \
                     and (wt.pos = t.pos or coalesce(wt.image_path, wt.text, '') = coalesce(t.image_path, t.text, '')) \
               ) \
             order by 19, 1",
            &[&window_id],
        )
        .await
    {
        Ok(v) => v,
        Err(_) => match client
            .query(
                "select b.reg_id, false as is_text, coalesce(b.x,20), coalesce(b.y,20), \
                        coalesce(b.w,120), coalesce(b.h,34), \
                        b.visible, b.writable, coalesce(r.name,''), \
                        coalesce(r.mb,0), coalesce(r.n_mb,0), coalesce(r.tip,0), r.bits, \
                        b.label_override, b.unit, b.fmt, b.scale_max, b.component_kind, b.pos \
                 from ui.kpz_window_reg_binding b \
                 join public.reg r on r.id = b.reg_id \
                 where b.window_id = $1 \
                 union all \
                 select -1 * (1000000 + t.pos) as reg_id, true as is_text, \
                        coalesce(t.x,20), coalesce(t.y,20), coalesce(t.w,120), coalesce(t.h,34), \
                        t.visible, false as writable, coalesce(t.text,''), \
                        0 as reg_mb, 0 as reg_n_mb, 0 as reg_tip, null::int as reg_bits, \
                        null::text as label_override, null::text as unit, null::text as fmt, \
                        null::double precision as scale_max, null::text as component_kind, t.pos \
                 from ui.kpz_window_text_item t \
                 where t.window_id = $1 \
                 order by 19, 1",
                &[&window_id],
            )
            .await
        {
            Ok(v) => v,
            Err(_) => {
                client
                    .query(
                        "select b.reg_id, false as is_text, coalesce(b.x,20), coalesce(b.y,20), \
                                coalesce(b.w,120), coalesce(b.h,34), \
                                b.visible, b.writable, coalesce(r.name,''), \
                                coalesce(r.mb,0), coalesce(r.n_mb,0), coalesce(r.tip,0), r.bits, \
                                b.label_override, b.unit, b.fmt, b.scale_max, b.component_kind, b.pos \
                         from ui.kpz_window_reg_binding b \
                         join public.reg r on r.id = b.reg_id \
                         where b.window_id = $1 \
                         order by b.pos, b.reg_id",
                        &[&window_id],
                    )
                    .await?
            }
        },
    };
    let out = rows
        .into_iter()
        .map(|r| UiBindingDto {
            reg_id: r.get::<_, i32>(0),
            is_text: r.get::<_, bool>(1),
            x: r.get::<_, i32>(2),
            y: r.get::<_, i32>(3),
            w: r.get::<_, i32>(4),
            h: r.get::<_, i32>(5),
            visible: r.get::<_, bool>(6),
            writable: r.get::<_, bool>(7),
            reg_name: r.get::<_, String>(8),
            reg_mb: r.get::<_, i32>(9),
            reg_n_mb: r.get::<_, i32>(10),
            reg_tip: r.get::<_, i32>(11),
            reg_bits: r.try_get::<_, Option<i32>>(12).ok().flatten(),
            label_override: r.try_get::<_, Option<String>>(13).ok().flatten(),
            unit: r.try_get::<_, Option<String>>(14).ok().flatten(),
            fmt: r.try_get::<_, Option<String>>(15).ok().flatten(),
            scale_max: r.try_get::<_, Option<f64>>(16).ok().flatten(),
            component_kind: r.try_get::<_, Option<String>>(17).ok().flatten(),
        })
        .collect();
    Ok(out)
}

pub async fn load_live_values(client: &Client, kpz_id: i32, reg_ids: &[i32]) -> Result<Vec<LiveValueDto>> {
    if reg_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = client
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
        .map(|r| LiveValueDto {
            reg_id: r.get::<_, i32>(0),
            val_num: r.try_get::<_, Option<f64>>(1).ok().flatten(),
        })
        .collect();
    Ok(out)
}

pub async fn load_alarm_rules_for_kpz(client: &Client, kpz_id: i32) -> Result<Vec<AlarmRulePreviewDto>> {
    let rows = client
        .query(
            "select reg_id, set_lo, set_hi, set_lo_1, set_hi_1 \
             from alarm_rule \
             where kpz_id = $1 and enabled = true \
             order by reg_id, id",
            &[&kpz_id],
        )
        .await?;
    let out = rows
        .into_iter()
        .map(|r| AlarmRulePreviewDto {
            reg_id: r.get::<_, i32>(0),
            set_lo: r.try_get::<_, Option<f64>>(1).ok().flatten(),
            set_hi: r.try_get::<_, Option<f64>>(2).ok().flatten(),
            set_lo_1: r.try_get::<_, Option<f64>>(3).ok().flatten(),
            set_hi_1: r.try_get::<_, Option<f64>>(4).ok().flatten(),
        })
        .collect();
    Ok(out)
}

fn parse_i32_or_default(s: &str, default: i32) -> i32 {
    s.trim().parse::<i32>().unwrap_or(default)
}

pub async fn load_io_conn_for_kpz(client: &Client, kpz_id: i32) -> Result<Option<IoConnDto>> {
    let kpz_row = match client
        .query_opt(
            "select coalesce(rtu,1), obj, coalesce(modem,50002), coalesce(max_pkt_len,800)
             from kpz where id = $1 limit 1",
            &[&kpz_id],
        )
        .await
    {
        Ok(v) => v,
        Err(_) => {
            client
                .query_opt(
                    "select coalesce(rtu,1), obj, 50002 as modem, 800 as max_pkt_len
                     from kpz where id = $1 limit 1",
                    &[&kpz_id],
                )
                .await?
        }
    };

    let Some(k) = kpz_row else {
        return Ok(None);
    };
    let rtu = k.get::<_, i32>(0).clamp(1, 65535) as u16;
    let obj_id = k.try_get::<_, Option<i32>>(1).ok().flatten();
    let modem = k.get::<_, i32>(2).clamp(0, 65535) as u16;
    let max_pkt_len = k.get::<_, i32>(3).max(256) as usize;

    let mut obj_name_raw: Option<String> = None;
    let mut obj_ip_raw: Option<String> = None;
    let mut ip_id: Option<i32> = None;
    let mut port_id: Option<i32> = None;
    let mut kanal_id: Option<i32> = None;
    let mut speed_id: Option<i32> = None;
    let mut stop_id: Option<i32> = None;
    let mut parit_id: Option<i32> = None;
    let mut bit_id: Option<i32> = None;
    if let Some(obj) = obj_id {
        if let Ok(Some(o)) = client
            .query_opt(
                "select coalesce(name,''), ip, port, kanal, speed, stop, parit, bit
                 from obj where id = $1 limit 1",
                &[&obj],
            )
            .await
        {
            obj_name_raw = o.try_get::<_, Option<String>>(0).ok().flatten();
            obj_ip_raw = o.try_get::<_, Option<String>>(1).ok().flatten();
            ip_id = obj_ip_raw
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .and_then(|s| s.parse::<i32>().ok());
            port_id = o.try_get::<_, Option<i32>>(2).ok().flatten();
            kanal_id = o.try_get::<_, Option<i32>>(3).ok().flatten();
            speed_id = o.try_get::<_, Option<i32>>(4).ok().flatten();
            stop_id = o.try_get::<_, Option<i32>>(5).ok().flatten();
            parit_id = o.try_get::<_, Option<i32>>(6).ok().flatten();
            bit_id = o.try_get::<_, Option<i32>>(7).ok().flatten();
        }
    }

    async fn dict_name(client: &Client, table: &str, id: Option<i32>) -> Option<String> {
        let id = id?;
        let table_ok = matches!(table, "ip" | "port" | "kanal" | "speed" | "stop" | "parit" | "bit");
        if !table_ok {
            return None;
        }
        let sql = format!("select coalesce(name,'') from {} where id = $1 limit 1", table);
        let row = client.query_opt(&sql, &[&id]).await.ok().flatten()?;
        row.try_get::<_, String>(0).ok()
    }

    let ip = dict_name(client, "ip", ip_id)
        .await
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            obj_ip_raw
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty() && s.parse::<i32>().is_err())
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            obj_name_raw
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = parse_i32_or_default(
        &dict_name(client, "port", port_id)
            .await
            .unwrap_or_else(|| "5100".to_string()),
        5100,
    )
    .clamp(1, 65535) as u16;
    let kan = parse_i32_or_default(
        &dict_name(client, "kanal", kanal_id)
            .await
            .unwrap_or_else(|| "3".to_string()),
        3,
    )
    .clamp(0, 255) as u8;
    let speed = parse_i32_or_default(
        &dict_name(client, "speed", speed_id)
            .await
            .unwrap_or_else(|| "8".to_string()),
        8,
    )
    .clamp(0, 255) as u8;
    let stop = parse_i32_or_default(
        &dict_name(client, "stop", stop_id)
            .await
            .unwrap_or_else(|| "0".to_string()),
        0,
    )
    .clamp(0, 255) as u8;
    let par = parse_i32_or_default(
        &dict_name(client, "parit", parit_id)
            .await
            .unwrap_or_else(|| "2".to_string()),
        2,
    )
    .clamp(0, 255) as u8;
    let data = parse_i32_or_default(
        &dict_name(client, "bit", bit_id)
            .await
            .unwrap_or_else(|| "8".to_string()),
        8,
    )
    .clamp(0, 255) as u8;

    Ok(Some(IoConnDto {
        ip,
        port,
        rtu,
        modem,
        kan,
        speed,
        stop,
        par,
        data,
        max_pkt_len,
    }))
}

pub async fn load_regs_io_by_ids(client: &Client, reg_ids: &[i32]) -> Result<Vec<RegIoDto>> {
    if reg_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = client
        .query(
            "select
                r.id,
                coalesce(r.mb, 0),
                coalesce(r.n_mb, 0),
                coalesce(r.tip, 0),
                r.bits
             from reg r
             where r.id = any($1)
             order by r.id",
            &[&reg_ids],
        )
        .await?;
    let out = rows
        .into_iter()
        .map(|r| RegIoDto {
            id: r.get::<_, i32>(0),
            mb: r.get::<_, i32>(1),
            n_mb_id: r.get::<_, i32>(2),
            tip: r.get::<_, i32>(3),
            bits: r.try_get::<_, Option<i32>>(4).ok().flatten(),
        })
        .collect();
    Ok(out)
}

pub async fn load_n_mb_dict(client: &Client) -> Result<HashMap<i32, String>> {
    let rows = client
        .query("select id, coalesce(name,'') from n_mb order by id", &[])
        .await?;
    let mut out = HashMap::new();
    for r in rows {
        out.insert(r.get::<_, i32>(0), r.get::<_, String>(1));
    }
    Ok(out)
}

fn decode_groups(grups: &[u8]) -> Vec<i32> {
    if grups.len() != 64 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for bit in 0..(64 * 8) {
        let byte_index = bit >> 3;
        let bit_index = bit & 7;
        if (grups[byte_index] & (1 << bit_index)) != 0 {
            out.push((bit + 1) as i32);
        }
    }
    out
}
