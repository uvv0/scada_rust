-- SS5 window groups + command buttons schema
-- Depends on:
-- 1) ui_schema.sql
-- 2) ss5_window_binding_schema.sql
create schema if not exists ui;

-- Selected groups for a window (defines which REG groups are available in editor/runtime view).
create table if not exists ui.kpz_window_group (
    window_id bigint not null references ui.kpz_window(id) on delete cascade,
    group_id int not null,
    pos int not null default 0,
    updated_at timestamptz not null default now(),
    primary key (window_id, group_id),
    unique (window_id, pos)
);

create index if not exists idx_kpz_window_group_group
    on ui.kpz_window_group (group_id);

-- Command buttons shown in the window.
create table if not exists ui.kpz_window_command_button (
    id bigserial primary key,
    window_id bigint not null references ui.kpz_window(id) on delete cascade,
    code text not null,                         -- stable key, e.g. 'start_pump', 'reset_alarm'
    title text not null,
    pos int not null default 0,
    style text not null default 'primary' check (style in ('primary', 'secondary', 'danger')),
    confirm_text text,
    is_active boolean not null default true,
    updated_at timestamptz not null default now(),
    unique (window_id, code),
    unique (window_id, pos)
);

create index if not exists idx_kpz_window_command_button_window
    on ui.kpz_window_command_button (window_id);

-- Button actions (one button can execute one or more commands in order).
create table if not exists ui.kpz_window_command_action (
    button_id bigint not null references ui.kpz_window_command_button(id) on delete cascade,
    action_no int not null,                     -- execution order: 1..N
    action_type text not null check (action_type in ('write_reg', 'post_cmd')),
    reg_id int references public.reg(id) on delete restrict, -- for write_reg
    func int check (func in (5, 6)),            -- for post_cmd
    addr_human int,                             -- for post_cmd
    value_num double precision,                 -- command value
    updated_at timestamptz not null default now(),
    primary key (button_id, action_no)
);

create index if not exists idx_kpz_window_command_action_reg
    on ui.kpz_window_command_action (reg_id);
