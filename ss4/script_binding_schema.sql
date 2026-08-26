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
