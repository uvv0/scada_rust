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

alter table scheduler_runtime_cfg add column if not exists metrics_p95_warn_ms bigint not null default 1000;
alter table scheduler_runtime_cfg add column if not exists metrics_p95_crit_ms bigint not null default 3000;
alter table scheduler_runtime_cfg add column if not exists modbus_a_timeout_ms bigint not null default 1800;
alter table scheduler_runtime_cfg add column if not exists modbus_script_timeout_ms bigint not null default 2600;

insert into scheduler_runtime_cfg (id, no_response_failures, no_response_backoff_sec, metrics_p95_warn_ms, metrics_p95_crit_ms, modbus_a_timeout_ms, modbus_script_timeout_ms)
values (1, 3, 600, 1000, 3000, 1800, 2600)
on conflict (id) do update set
    no_response_failures = excluded.no_response_failures,
    no_response_backoff_sec = excluded.no_response_backoff_sec,
    metrics_p95_warn_ms = excluded.metrics_p95_warn_ms,
    metrics_p95_crit_ms = excluded.metrics_p95_crit_ms,
    modbus_a_timeout_ms = excluded.modbus_a_timeout_ms,
    modbus_script_timeout_ms = excluded.modbus_script_timeout_ms,
    updated_at = now();
