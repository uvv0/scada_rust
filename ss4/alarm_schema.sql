-- Alarm rules/state/events schema for ss4
-- Apply on target DB (e.g. postgres_restored) before enabling alarm checks.

-- Enable/disable A-mode alarm POST hook per KPZ.
alter table if exists public.kpz
    add column if not exists en_post boolean not null default false;

create table if not exists public.alarm_rule (
    id bigserial primary key,
    kpz_id int not null,
    reg_id int not null,
    enabled boolean not null default true,
    cmp text not null check (cmp in ('lt', 'le', 'gt', 'ge', 'lt_1', 'le_1', 'gt_1', 'ge_1', 'between', 'outside')),
    set_lo double precision,
    set_hi double precision,
    set_lo_1 double precision,
    set_hi_1 double precision,
    hysteresis double precision not null default 0,
    on_delay_sec int not null default 0,
    off_delay_sec int not null default 0,
    severity smallint not null default 1,
    code text,
    message text,
    updated_at timestamptz not null default now()
);

alter table if exists public.alarm_rule
    add column if not exists set_lo_1 double precision;

alter table if exists public.alarm_rule
    add column if not exists set_hi_1 double precision;

do $$
begin
    if exists (
        select 1
        from pg_constraint
        where conname = 'alarm_rule_cmp_check'
          and conrelid = 'public.alarm_rule'::regclass
    ) then
        alter table public.alarm_rule drop constraint alarm_rule_cmp_check;
    end if;
end $$;

alter table if exists public.alarm_rule
    add constraint alarm_rule_cmp_check
    check (cmp in ('lt', 'le', 'gt', 'ge', 'lt_1', 'le_1', 'gt_1', 'ge_1', 'between', 'outside'));

do $$
begin
    if exists (
        select 1
        from pg_constraint
        where conname = 'alarm_rule_prewarn_bounds_check'
          and conrelid = 'public.alarm_rule'::regclass
    ) then
        alter table public.alarm_rule drop constraint alarm_rule_prewarn_bounds_check;
    end if;
end $$;

alter table if exists public.alarm_rule
    add constraint alarm_rule_prewarn_bounds_check
    check (
        (set_lo is null or set_lo_1 is null or set_lo_1 > set_lo)
        and
        (set_hi is null or set_hi_1 is null or set_hi_1 < set_hi)
    );

create index if not exists idx_alarm_rule_kpz_reg_enabled
    on public.alarm_rule (kpz_id, reg_id)
    where enabled = true;

create table if not exists public.alarm_state (
    rule_id bigint primary key references public.alarm_rule(id) on delete cascade,
    active boolean not null default false,
    active_since timestamptz,
    last_value double precision,
    updated_at timestamptz not null default now()
);

create table if not exists public.alarm_event (
    id bigserial primary key,
    ts timestamptz not null default now(),
    kpz_id int not null,
    reg_id int not null,
    rule_id bigint not null references public.alarm_rule(id) on delete cascade,
    event text not null check (event in ('on', 'off')),
    value double precision,
    set_lo double precision,
    set_hi double precision,
    severity smallint not null default 1,
    code text,
    message text
);

create index if not exists idx_alarm_event_ts
    on public.alarm_event (ts desc);

create index if not exists idx_alarm_event_kpz_ts
    on public.alarm_event (kpz_id, ts desc);
