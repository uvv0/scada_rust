-- ss4 Database Initialization Script
-- Run against a fresh PostgreSQL database.
-- Usage: psql -d ss4_db -f init_db.sql

-- Lookup tables
create table if not exists ip (
    id serial primary key,
    name text not null unique
);

create table if not exists port (
    id serial primary key,
    name text not null unique
);

create table if not exists n_mb (
    id serial primary key,
    name text not null unique
);

-- Objects (physical devices / connection endpoints)
create table if not exists obj (
    id serial primary key,
    name text,
    ip text,
    port text,
    kanal integer,
    speed integer,
    stop integer,
    parit integer,
    bit integer
);

-- Control points (logical polling groups)
create table if not exists kpz (
    id serial primary key,
    name text,
    rtu integer not null,
    obj integer not null references obj(id),
    modem integer not null,
    grups bytea,
    max_pkt_len integer not null default 256,
    start integer not null default 0,
    t_a integer not null default 6000,
    t_script integer not null default 6000,
    en_post boolean not null default false,
    updated_at timestamptz not null default now()
);

create index if not exists idx_kpz_obj on kpz(obj);

-- Registers (data points)
create table if not exists reg (
    id serial primary key,
    name text not null default '',
    addr integer not null,
    n_mb integer references n_mb(id),
    tip integer not null default 0,
    bits integer,
    grup integer,
    a_en boolean not null default false,
    a_no_write integer not null default 0,
    updated_at timestamptz not null default now()
);

create index if not exists idx_reg_addr on reg(addr);
create index if not exists idx_reg_grup on reg(grup);

-- Group scripts (Lua pre/post processing)
create table if not exists g_script (
    grup integer primary key,
    pre_src text,
    post_src text,
    max_k integer,
    max_words integer,
    en boolean,
    ver integer,
    updated_at timestamptz not null default now()
);

-- Script bindings (logical -> physical register mapping)
create table if not exists script_binding (
    id bigserial primary key,
    kpz_id integer not null references kpz(id) on delete cascade,
    grup integer not null,
    logical integer not null,
    reg_id integer null references reg(id) on delete set null,
    addr integer null,
    enabled boolean not null default true,
    updated_at timestamptz not null default now(),
    unique (kpz_id, grup, logical)
);

create index if not exists ix_script_binding_kpz_grup
    on script_binding(kpz_id, grup);

create index if not exists ix_script_binding_reg_id
    on script_binding(reg_id);

-- Archive values
create table if not exists arx_val (
    kpz_id integer not null,
    reg_id integer not null,
    ts_unix bigint not null,
    tip integer not null,
    val_num double precision,
    val_raw bytea,
    primary key (kpz_id, reg_id, ts_unix)
);

create index if not exists idx_arx_val_ts on arx_val(ts_unix desc);
create index if not exists idx_arx_val_kpz_reg on arx_val(kpz_id, reg_id);

-- Archive state tracking
create table if not exists arx_state (
    kpz_id integer not null,
    arx_id integer not null,
    last_ind integer not null default 0,
    updated_at timestamptz not null default now(),
    primary key (kpz_id, arx_id)
);

-- Polling log
create table if not exists poll_log (
    id bigserial primary key,
    ts timestamptz not null default now(),
    kpz_id integer not null references kpz(id) on delete cascade,
    kind text not null,
    msg text
);

create index if not exists idx_poll_log_ts on poll_log(ts desc);
create index if not exists idx_poll_log_kpz on poll_log(kpz_id);

-- Delta write queue (elam)
create table if not exists elam (
    ctid tid not null,
    kpz_id integer not null,
    reg_id integer not null,
    val_num double precision,
    val_raw bytea,
    ts timestamptz not null default now()
);

create index if not exists idx_elam_kpz_reg on elam(kpz_id, reg_id);

-- Scheduler runtime configuration
create table if not exists scheduler_runtime_cfg (
    id bigserial primary key,
    no_response_failures integer not null default 3,
    no_response_backoff_sec bigint not null default 600,
    metrics_p95_warn_ms bigint not null default 1000,
    metrics_p95_crit_ms bigint not null default 3000,
    modbus_a_timeout_ms bigint not null default 1800,
    modbus_script_timeout_ms bigint not null default 2600,
    updated_at timestamptz not null default now()
);

insert into scheduler_runtime_cfg
    (id, no_response_failures, no_response_backoff_sec,
     metrics_p95_warn_ms, metrics_p95_crit_ms,
     modbus_a_timeout_ms, modbus_script_timeout_ms)
values (1, 3, 600, 1000, 3000, 1800, 2600)
on conflict (id) do nothing;

-- Seed data
insert into ip (name) values ('127.0.0.1') on conflict (name) do nothing;
insert into port (name) values ('502') on conflict (name) do nothing;
insert into n_mb (name) values ('default') on conflict (name) do nothing;
