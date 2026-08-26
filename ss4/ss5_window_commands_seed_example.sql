-- SS5 demo seed: one window, groups, reg bindings, command buttons/actions.
-- Requires:
-- 1) ui_schema.sql
-- 2) ss5_window_binding_schema.sql
-- 3) ss5_window_commands_schema.sql
do $$
declare
    v_kpz_id int;
    v_window_id bigint;
    v_reg1 int;
    v_reg2 int;
begin
    select id into v_kpz_id
    from public.kpz
    order by id
    limit 1;

    if v_kpz_id is null then
        raise notice 'seed skipped: no rows in public.kpz';
        return;
    end if;

    insert into ui.kpz_window (kpz_id, code, title, description, is_active)
    values (v_kpz_id, 'main', 'Main Window', 'Demo window for SS5 editor', true)
    on conflict (kpz_id, code) do update
    set
        title = excluded.title,
        description = excluded.description,
        is_active = excluded.is_active,
        updated_at = now()
    returning id into v_window_id;

    if v_window_id is null then
        select id into v_window_id
        from ui.kpz_window
        where kpz_id = v_kpz_id and code = 'main'
        limit 1;
    end if;

    -- Demo groups for this window.
    insert into ui.kpz_window_group (window_id, group_id, pos)
    values
        (v_window_id, 1, 10),
        (v_window_id, 2, 20)
    on conflict (window_id, group_id) do update
    set
        pos = excluded.pos,
        updated_at = now();

    -- Select first two regs for demo bindings.
    select r.id into v_reg1
    from public.reg r
    order by r.id
    limit 1;

    select r.id into v_reg2
    from public.reg r
    order by r.id
    offset 1
    limit 1;

    if v_reg1 is not null then
        insert into ui.kpz_window_reg_binding (
            window_id, reg_id, pos, visible, writable, label_override, unit, fmt
        )
        values
            (v_window_id, v_reg1, 10, true, false, 'Demo Reg 1', null, '0.00')
        on conflict (window_id, reg_id) do update
        set
            pos = excluded.pos,
            visible = excluded.visible,
            writable = excluded.writable,
            label_override = excluded.label_override,
            unit = excluded.unit,
            fmt = excluded.fmt,
            updated_at = now();
    end if;

    if v_reg2 is not null then
        insert into ui.kpz_window_reg_binding (
            window_id, reg_id, pos, visible, writable, label_override, unit, fmt
        )
        values
            (v_window_id, v_reg2, 20, true, false, 'Demo Reg 2', null, '0.00')
        on conflict (window_id, reg_id) do update
        set
            pos = excluded.pos,
            visible = excluded.visible,
            writable = excluded.writable,
            label_override = excluded.label_override,
            unit = excluded.unit,
            fmt = excluded.fmt,
            updated_at = now();
    end if;

    -- Demo button: start
    insert into ui.kpz_window_command_button (window_id, code, title, pos, style, confirm_text, is_active)
    values (v_window_id, 'start_unit', 'Start', 10, 'primary', 'Start unit?', true)
    on conflict (window_id, code) do update
    set
        title = excluded.title,
        pos = excluded.pos,
        style = excluded.style,
        confirm_text = excluded.confirm_text,
        is_active = excluded.is_active,
        updated_at = now();

    -- Demo button: stop
    insert into ui.kpz_window_command_button (window_id, code, title, pos, style, confirm_text, is_active)
    values (v_window_id, 'stop_unit', 'Stop', 20, 'danger', 'Stop unit?', true)
    on conflict (window_id, code) do update
    set
        title = excluded.title,
        pos = excluded.pos,
        style = excluded.style,
        confirm_text = excluded.confirm_text,
        is_active = excluded.is_active,
        updated_at = now();

    -- Rebuild actions for start button.
    delete from ui.kpz_window_command_action a
    using ui.kpz_window_command_button b
    where a.button_id = b.id
      and b.window_id = v_window_id
      and b.code = 'start_unit';

    insert into ui.kpz_window_command_action (button_id, action_no, action_type, func, addr_human, value_num)
    select b.id, 1, 'post_cmd', 5, 512, 1.0
    from ui.kpz_window_command_button b
    where b.window_id = v_window_id and b.code = 'start_unit';

    -- Rebuild actions for stop button.
    delete from ui.kpz_window_command_action a
    using ui.kpz_window_command_button b
    where a.button_id = b.id
      and b.window_id = v_window_id
      and b.code = 'stop_unit';

    insert into ui.kpz_window_command_action (button_id, action_no, action_type, func, addr_human, value_num)
    select b.id, 1, 'post_cmd', 5, 512, 0.0
    from ui.kpz_window_command_button b
    where b.window_id = v_window_id and b.code = 'stop_unit';

    raise notice 'ss5 demo seed applied: kpz_id=%, window_id=%', v_kpz_id, v_window_id;
end $$;
