use tokio_postgres::Client;

pub(crate) const SS7_UI_SCHEMA_SQL: &str = r#"
alter table if exists public.kpz add column if not exists en_post boolean not null default false;

create schema if not exists ui;

create table if not exists ui.kpz_window (
    id bigserial primary key,
    kpz_id int not null references public.kpz(id) on delete cascade,
    code text not null,
    title text not null,
    description text,
    is_active boolean not null default true,
    updated_at timestamptz not null default now(),
    unique (kpz_id, code)
);

create index if not exists idx_kpz_window_kpz
    on ui.kpz_window (kpz_id);
create unique index if not exists ux_kpz_window_kpz_code
    on ui.kpz_window (kpz_id, code);

alter table if exists ui.kpz_window
    add column if not exists description text;
alter table if exists ui.kpz_window
    add column if not exists is_active boolean not null default true;
alter table if exists ui.kpz_window
    add column if not exists updated_at timestamptz not null default now();

create table if not exists ui.kpz_window_reg_binding (
    window_id bigint not null references ui.kpz_window(id) on delete cascade,
    reg_id int not null references public.reg(id) on delete restrict,
    pos int not null default 0,
    x int not null default 20,
    y int not null default 20,
    w int not null default 120,
    h int not null default 34,
    visible boolean not null default true,
    writable boolean not null default false,
    label_override text,
    unit text,
    fmt text,
    scale_max double precision,
    web_safe_muted boolean not null default false,
    updated_at timestamptz not null default now(),
    primary key (window_id, reg_id),
    unique (window_id, pos)
);
create unique index if not exists ux_kpz_window_reg_binding_window_pos
    on ui.kpz_window_reg_binding (window_id, pos);

alter table if exists ui.kpz_window_reg_binding
    add column if not exists x int not null default 20;
alter table if exists ui.kpz_window_reg_binding
    add column if not exists y int not null default 20;
alter table if exists ui.kpz_window_reg_binding
    add column if not exists w int not null default 120;
alter table if exists ui.kpz_window_reg_binding
    add column if not exists h int not null default 34;
alter table if exists ui.kpz_window_reg_binding
    add column if not exists component_kind text;
alter table if exists ui.kpz_window_reg_binding
    add column if not exists scale_max double precision;
alter table if exists ui.kpz_window_reg_binding
    add column if not exists web_safe_muted boolean not null default false;

create table if not exists ui.kpz_window_template (
    id bigserial primary key,
    code text not null unique,
    title text not null,
    description text,
    source_window_id bigint references ui.kpz_window(id) on delete set null,
    is_active boolean not null default true,
    updated_at timestamptz not null default now()
);
create unique index if not exists ux_kpz_window_template_code
    on ui.kpz_window_template (code);

alter table if exists ui.kpz_window_template
    add column if not exists description text;
alter table if exists ui.kpz_window_template
    add column if not exists source_window_id bigint references ui.kpz_window(id) on delete set null;
alter table if exists ui.kpz_window_template
    add column if not exists is_active boolean not null default true;
alter table if exists ui.kpz_window_template
    add column if not exists updated_at timestamptz not null default now();

create table if not exists ui.kpz_window_template_binding (
    template_id bigint not null references ui.kpz_window_template(id) on delete cascade,
    reg_id int not null references public.reg(id) on delete restrict,
    pos int not null default 0,
    x int not null default 20,
    y int not null default 20,
    w int not null default 120,
    h int not null default 34,
    visible boolean not null default true,
    writable boolean not null default false,
    label_override text,
    unit text,
    fmt text,
    scale_max double precision,
    web_safe_muted boolean not null default false,
    updated_at timestamptz not null default now(),
    primary key (template_id, reg_id),
    unique (template_id, pos)
);
create unique index if not exists ux_kpz_window_template_binding_template_pos
    on ui.kpz_window_template_binding (template_id, pos);
alter table if exists ui.kpz_window_template_binding
    add column if not exists component_kind text;
alter table if exists ui.kpz_window_template_binding
    add column if not exists scale_max double precision;
alter table if exists ui.kpz_window_template_binding
    add column if not exists web_safe_muted boolean not null default false;

create table if not exists ui.kpz_template_link (
    kpz_id int not null references public.kpz(id) on delete cascade,
    template_id bigint not null references ui.kpz_window_template(id) on delete cascade,
    is_default boolean not null default false,
    sort_order int not null default 0,
    updated_at timestamptz not null default now(),
    primary key (kpz_id, template_id)
);
create unique index if not exists ux_kpz_template_link_kpz_template
    on ui.kpz_template_link (kpz_id, template_id);

alter table if exists ui.kpz_template_link
    add column if not exists is_default boolean not null default false;
alter table if exists ui.kpz_template_link
    add column if not exists sort_order int not null default 0;
alter table if exists ui.kpz_template_link
    add column if not exists updated_at timestamptz not null default now();

create index if not exists idx_kpz_template_link_kpz
    on ui.kpz_template_link (kpz_id, sort_order, template_id);

create table if not exists ui.kp_template (
    id bigserial primary key,
    code text not null unique,
    title text not null,
    description text,
    is_active boolean not null default true,
    updated_at timestamptz not null default now()
);
create unique index if not exists ux_kp_template_code
    on ui.kp_template (code);

alter table if exists ui.kp_template
    add column if not exists description text;
alter table if exists ui.kp_template
    add column if not exists is_active boolean not null default true;
alter table if exists ui.kp_template
    add column if not exists updated_at timestamptz not null default now();

create table if not exists ui.kp_template_window (
    kp_template_id bigint not null references ui.kp_template(id) on delete cascade,
    window_template_id bigint not null references ui.kpz_window_template(id) on delete restrict,
    sort_order int not null default 0,
    is_default boolean not null default false,
    updated_at timestamptz not null default now(),
    primary key (kp_template_id, window_template_id)
);
create unique index if not exists ux_kp_template_window_pair
    on ui.kp_template_window (kp_template_id, window_template_id);

alter table if exists ui.kp_template_window
    add column if not exists sort_order int not null default 0;
alter table if exists ui.kp_template_window
    add column if not exists is_default boolean not null default false;
alter table if exists ui.kp_template_window
    add column if not exists updated_at timestamptz not null default now();

create index if not exists idx_kp_template_window_kp_template
    on ui.kp_template_window (kp_template_id, sort_order, window_template_id);

create table if not exists ui.kpz_kp_template_link (
    kpz_id int not null references public.kpz(id) on delete cascade,
    kp_template_id bigint not null references ui.kp_template(id) on delete restrict,
    updated_at timestamptz not null default now(),
    primary key (kpz_id)
);
create unique index if not exists ux_kpz_kp_template_link_kpz
    on ui.kpz_kp_template_link (kpz_id);

alter table if exists ui.kpz_kp_template_link
    add column if not exists updated_at timestamptz not null default now();

create table if not exists ui.kpz_window_text_item (
    id bigserial primary key,
    window_id bigint not null references ui.kpz_window(id) on delete cascade,
    pos int not null default 0,
    x int not null default 20,
    y int not null default 20,
    w int not null default 120,
    h int not null default 34,
    visible boolean not null default true,
    text text not null default '',
    item_kind text not null default 'text',
    image_path text,
    fit_mode text not null default 'contain',
    opacity double precision not null default 1.0,
    web_safe_muted boolean not null default false,
    updated_at timestamptz not null default now(),
    unique (window_id, pos)
);
create unique index if not exists ux_kpz_window_text_item_window_pos
    on ui.kpz_window_text_item (window_id, pos);

alter table if exists ui.kpz_window_text_item
    add column if not exists x int not null default 20;
alter table if exists ui.kpz_window_text_item
    add column if not exists y int not null default 20;
alter table if exists ui.kpz_window_text_item
    add column if not exists w int not null default 120;
alter table if exists ui.kpz_window_text_item
    add column if not exists h int not null default 34;
alter table if exists ui.kpz_window_text_item
    add column if not exists visible boolean not null default true;
alter table if exists ui.kpz_window_text_item
    add column if not exists text text not null default '';
alter table if exists ui.kpz_window_text_item
    add column if not exists item_kind text not null default 'text';
alter table if exists ui.kpz_window_text_item
    add column if not exists image_path text;
alter table if exists ui.kpz_window_text_item
    add column if not exists fit_mode text not null default 'contain';
alter table if exists ui.kpz_window_text_item
    add column if not exists opacity double precision not null default 1.0;
alter table if exists ui.kpz_window_text_item
    add column if not exists web_safe_muted boolean not null default false;
alter table if exists ui.kpz_window_text_item
    add column if not exists updated_at timestamptz not null default now();

create table if not exists ui.kpz_window_template_text_item (
    id bigserial primary key,
    template_id bigint not null references ui.kpz_window_template(id) on delete cascade,
    pos int not null default 0,
    x int not null default 20,
    y int not null default 20,
    w int not null default 120,
    h int not null default 34,
    visible boolean not null default true,
    text text not null default '',
    item_kind text not null default 'text',
    image_path text,
    fit_mode text not null default 'contain',
    opacity double precision not null default 1.0,
    web_safe_muted boolean not null default false,
    updated_at timestamptz not null default now(),
    unique (template_id, pos)
);
create unique index if not exists ux_kpz_window_template_text_item_template_pos
    on ui.kpz_window_template_text_item (template_id, pos);

alter table if exists ui.kpz_window_template_text_item
    add column if not exists x int not null default 20;
alter table if exists ui.kpz_window_template_text_item
    add column if not exists y int not null default 20;
alter table if exists ui.kpz_window_template_text_item
    add column if not exists w int not null default 120;
alter table if exists ui.kpz_window_template_text_item
    add column if not exists h int not null default 34;
alter table if exists ui.kpz_window_template_text_item
    add column if not exists visible boolean not null default true;
alter table if exists ui.kpz_window_template_text_item
    add column if not exists text text not null default '';
alter table if exists ui.kpz_window_template_text_item
    add column if not exists item_kind text not null default 'text';
alter table if exists ui.kpz_window_template_text_item
    add column if not exists image_path text;
alter table if exists ui.kpz_window_template_text_item
    add column if not exists fit_mode text not null default 'contain';
alter table if exists ui.kpz_window_template_text_item
    add column if not exists opacity double precision not null default 1.0;
alter table if exists ui.kpz_window_template_text_item
    add column if not exists web_safe_muted boolean not null default false;
alter table if exists ui.kpz_window_template_text_item
    add column if not exists updated_at timestamptz not null default now();

create table if not exists public.web_users (
    id bigserial primary key,
    login text not null unique,
    password_salt text not null,
    password_hash text not null,
    role text not null default 'viewer',
    enabled boolean not null default true,
    created_at timestamptz not null default now()
);
create unique index if not exists ux_web_users_login
    on public.web_users (login);

alter table if exists public.web_users
    add column if not exists role text not null default 'viewer';
alter table if exists public.web_users
    add column if not exists enabled boolean not null default true;
alter table if exists public.web_users
    add column if not exists kpz_from int;
alter table if exists public.web_users
    add column if not exists kpz_to int;
alter table if exists public.web_users
    add column if not exists created_at timestamptz not null default now();
"#;

pub(crate) async fn apply_schema_migrations(client: &Client) -> anyhow::Result<()> {
    client.batch_execute(SS7_UI_SCHEMA_SQL).await?;
    Ok(())
}
