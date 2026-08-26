-- SS5 window binding schema
-- Purpose: bind REG set to a concrete KPZ window layout in UI.
-- Depends on ui schema from ui_schema.sql
create schema if not exists ui;

-- UI window descriptor for one KPZ.
create table if not exists ui.kpz_window (
    id bigserial primary key,
    kpz_id int not null references public.kpz(id) on delete cascade,
    code text not null,                         -- stable key, e.g. 'main', 'archive', 'alarms'
    title text not null,
    description text,
    is_active boolean not null default true,
    updated_at timestamptz not null default now(),
    unique (kpz_id, code)
);

create index if not exists idx_kpz_window_kpz
    on ui.kpz_window (kpz_id);

-- Binding of REG rows to a concrete window.
create table if not exists ui.kpz_window_reg_binding (
    window_id bigint not null references ui.kpz_window(id) on delete cascade,
    reg_id int not null references public.reg(id) on delete restrict,
    pos int not null default 0,                 -- visual order in grid
    visible boolean not null default true,
    writable boolean not null default false,    -- UI-level write toggle (ACL still applies)
    label_override text,                        -- optional caption override in UI
    unit text,                                  -- e.g. 'C', '%', 'bar'
    fmt text,                                   -- e.g. '0.00'
    updated_at timestamptz not null default now(),
    primary key (window_id, reg_id),
    unique (window_id, pos)
);

create index if not exists idx_kpz_window_reg_binding_reg
    on ui.kpz_window_reg_binding (reg_id);

-- Optional quick presets for window filter/sort/columns.
-- (Can reuse ui.ui_preset; this table is specific for binding editor state)
create table if not exists ui.kpz_window_editor_state (
    user_id text not null,
    window_id bigint not null references ui.kpz_window(id) on delete cascade,
    state jsonb not null default '{}'::jsonb,
    updated_at timestamptz not null default now(),
    primary key (user_id, window_id)
);
