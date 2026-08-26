-- Minimal seed example for UI schema.
-- Safe defaults: uses existing kpz/reg rows if present and writes only to ui.* tables.
-- Example user id for local/dev UI sessions.
-- Replace with your real auth subject in production.

-- 1) Give user read access to first available KPZ and write disabled by default.
insert into ui.user_kpz_access (user_id, kpz_id, can_read, can_write)
select
    'demo_user',
    k.id,
    true,
    false
from public.kpz k
order by k.id
limit 1
on conflict (user_id, kpz_id) do update
set
    can_read = excluded.can_read,
    can_write = excluded.can_write,
    updated_at = now();

-- 2) Optional per-reg write grant for the same user (first 5 regs as example).
insert into ui.user_reg_access (user_id, reg_id, can_read, can_write)
select
    'demo_user',
    r.id,
    true,
    false
from public.reg r
order by r.id
limit 5
on conflict (user_id, reg_id) do update
set
    can_read = excluded.can_read,
    can_write = excluded.can_write,
    updated_at = now();

-- 3) Default window preset for KPZ regs screen.
insert into ui.ui_preset (user_id, screen, kpz_id, title, config, is_default)
select
    'demo_user',
    'kpz_regs_window',
    k.id,
    'Default',
    jsonb_build_object(
        'columns', jsonb_build_array('id', 'name', 'addr', 'tip', 'value', 'ts', 'quality'),
        'sort', jsonb_build_object('by', 'addr', 'dir', 'asc'),
        'filters', jsonb_build_object('showWritableOnly', false, 'search', '')
    ),
    true
from public.kpz k
order by k.id
limit 1
on conflict (user_id, screen, kpz_id, title) do update
set
    config = excluded.config,
    is_default = excluded.is_default,
    updated_at = now();
