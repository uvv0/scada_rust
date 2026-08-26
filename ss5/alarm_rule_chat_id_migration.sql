alter table if exists public.alarm_rule
    add column if not exists chat_id text,
    add column if not exists tg_on_on boolean not null default false,
    add column if not exists tg_on_off boolean not null default false,
    add column if not exists tg_thr_main boolean not null default false,
    add column if not exists tg_thr_lvl1 boolean not null default false;

alter table if exists public.alarm_rule
    alter column tg_on_on set default false,
    alter column tg_on_off set default false,
    alter column tg_thr_main set default false,
    alter column tg_thr_lvl1 set default false;
