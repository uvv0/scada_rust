-- SS5 group-window composition schema
-- Purpose:
-- 1) Define reusable REG layouts per group.
-- 2) Compose KPZ window from multiple group layouts.
-- 3) Provide compatibility read view with resolved bindings.
--
-- Depends on:
-- 1) ui_schema.sql
-- 2) ss5_window_binding_schema.sql
create schema if not exists ui;

-- Reusable window template for one group.
create table if not exists ui.group_window (
    id bigserial primary key,
    group_id int not null,
    code text not null,                         -- stable key inside group
    title text not null,
    description text,
    is_active boolean not null default true,
    updated_at timestamptz not null default now(),
    unique (group_id, code)
);

create index if not exists idx_group_window_group
    on ui.group_window (group_id);

-- Binding of REG rows to a group template window.
create table if not exists ui.group_window_reg_binding (
    group_window_id bigint not null references ui.group_window(id) on delete cascade,
    reg_id int not null references public.reg(id) on delete restrict,
    pos int not null default 0,
    visible boolean not null default true,
    writable boolean not null default false,
    label_override text,
    unit text,
    fmt text,
    updated_at timestamptz not null default now(),
    primary key (group_window_id, reg_id),
    unique (group_window_id, pos)
);

create index if not exists idx_group_window_reg_binding_reg
    on ui.group_window_reg_binding (reg_id);

-- Composition of KPZ window from reusable group template windows.
create table if not exists ui.kpz_window_group_window (
    kpz_window_id bigint not null references ui.kpz_window(id) on delete cascade,
    group_window_id bigint not null references ui.group_window(id) on delete restrict,
    pos int not null default 0,                 -- section order inside KPZ window
    is_active boolean not null default true,
    updated_at timestamptz not null default now(),
    primary key (kpz_window_id, group_window_id),
    unique (kpz_window_id, pos)
);

create index if not exists idx_kpz_window_group_window_group_window
    on ui.kpz_window_group_window (group_window_id);

-- Compatibility read view for GET /windows/{window_id}/bindings.
-- Merge priority:
-- 1) legacy kpz_window_reg_binding row (override)
-- 2) composed group_window_reg_binding row
create or replace view ui.v_kpz_window_binding_resolved as
with from_group as (
    select
        c.kpz_window_id as window_id,
        b.reg_id,
        (c.pos * 100000 + b.pos) as pos,
        b.visible,
        b.writable,
        b.label_override,
        b.unit,
        b.fmt,
        'group'::text as source_kind,
        1 as source_priority
    from ui.kpz_window_group_window c
    join ui.group_window_reg_binding b
      on b.group_window_id = c.group_window_id
    where c.is_active = true
),
from_legacy as (
    select
        b.window_id,
        b.reg_id,
        b.pos,
        b.visible,
        b.writable,
        b.label_override,
        b.unit,
        b.fmt,
        'legacy'::text as source_kind,
        2 as source_priority
    from ui.kpz_window_reg_binding b
),
merged as (
    select * from from_group
    union all
    select * from from_legacy
),
ranked as (
    select
        m.*,
        row_number() over (
            partition by m.window_id, m.reg_id
            order by m.source_priority desc, m.pos
        ) as rn
    from merged m
)
select
    window_id,
    reg_id,
    pos,
    visible,
    writable,
    label_override,
    unit,
    fmt,
    source_kind
from ranked
where rn = 1;
