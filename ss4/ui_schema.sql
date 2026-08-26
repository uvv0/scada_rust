-- UI access/preset/audit schema for ss4
-- Apply on target DB (e.g. postgres_restored) before Windows/Web UI rollout.
create schema if not exists ui;

-- Access control: user -> kpz
create table if not exists ui.user_kpz_access (
    user_id text not null,
    kpz_id int not null references public.kpz(id) on delete cascade,
    can_read boolean not null default true,
    can_write boolean not null default false,
    updated_at timestamptz not null default now(),
    primary key (user_id, kpz_id)
);

create index if not exists idx_user_kpz_access_kpz
    on ui.user_kpz_access (kpz_id);

-- Optional granular access: user -> reg
create table if not exists ui.user_reg_access (
    user_id text not null,
    reg_id int not null references public.reg(id) on delete cascade,
    can_read boolean not null default true,
    can_write boolean not null default false,
    updated_at timestamptz not null default now(),
    primary key (user_id, reg_id)
);

create index if not exists idx_user_reg_access_reg
    on ui.user_reg_access (reg_id);

-- User UI presets (filters/sort/columns etc.)
create table if not exists ui.ui_preset (
    id bigserial primary key,
    user_id text not null,
    screen text not null,               -- e.g. 'kpz_regs_window'
    kpz_id int references public.kpz(id) on delete cascade,
    title text,
    config jsonb not null default '{}'::jsonb,
    is_default boolean not null default false,
    updated_at timestamptz not null default now(),
    unique (user_id, screen, kpz_id, title)
);

create index if not exists idx_ui_preset_lookup
    on ui.ui_preset (user_id, screen, kpz_id);

-- Audit of write attempts from UI/API
create table if not exists ui.reg_write_audit (
    id bigserial primary key,
    ts timestamptz not null default now(),
    user_id text not null,
    kpz_id int not null references public.kpz(id) on delete restrict,
    reg_id int not null references public.reg(id) on delete restrict,
    old_val double precision,
    new_val double precision,
    status text not null check (status in ('ok', 'denied', 'error')),
    err text
);

create index if not exists idx_reg_write_audit_ts
    on ui.reg_write_audit (ts desc);

create index if not exists idx_reg_write_audit_kpz_ts
    on ui.reg_write_audit (kpz_id, ts desc);
