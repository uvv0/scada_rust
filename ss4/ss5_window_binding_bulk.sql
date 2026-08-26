-- Bulk save helper for SS5 window bindings.
-- Usage target: API PUT /windows/{window_id}/bindings
-- Input format:
-- {
--   "bindings": [
--     {"reg_id":7001,"pos":10,"visible":true,"writable":false,"label_override":"T","unit":"C","fmt":"0.0"}
--   ]
-- }
create schema if not exists ui;

create or replace function ui.save_window_bindings(
    p_window_id bigint,
    p_bindings jsonb
)
returns void
language plpgsql
as $$
declare
    v_items jsonb;
begin
    v_items := coalesce(p_bindings -> 'bindings', '[]'::jsonb);
    if jsonb_typeof(v_items) <> 'array' then
        raise exception 'save_window_bindings: "bindings" must be JSON array';
    end if;

    -- Basic duplicate guards for deterministic ordering.
    if exists (
        select 1
        from (
            select (x->>'reg_id')::int as reg_id, count(*) as c
            from jsonb_array_elements(v_items) x
            group by (x->>'reg_id')::int
            having count(*) > 1
        ) d
    ) then
        raise exception 'save_window_bindings: duplicate reg_id in payload';
    end if;

    if exists (
        select 1
        from (
            select (x->>'pos')::int as pos, count(*) as c
            from jsonb_array_elements(v_items) x
            group by (x->>'pos')::int
            having count(*) > 1
        ) d
    ) then
        raise exception 'save_window_bindings: duplicate pos in payload';
    end if;

    -- Upsert all payload rows.
    insert into ui.kpz_window_reg_binding as t (
        window_id, reg_id, pos, visible, writable, label_override, unit, fmt, updated_at
    )
    select
        p_window_id as window_id,
        (x->>'reg_id')::int as reg_id,
        (x->>'pos')::int as pos,
        coalesce((x->>'visible')::boolean, true) as visible,
        coalesce((x->>'writable')::boolean, false) as writable,
        nullif(x->>'label_override', '') as label_override,
        nullif(x->>'unit', '') as unit,
        nullif(x->>'fmt', '') as fmt,
        now() as updated_at
    from jsonb_array_elements(v_items) x
    on conflict (window_id, reg_id) do update
    set
        pos = excluded.pos,
        visible = excluded.visible,
        writable = excluded.writable,
        label_override = excluded.label_override,
        unit = excluded.unit,
        fmt = excluded.fmt,
        updated_at = now();

    -- Remove rows absent in payload (full replacement semantics).
    delete from ui.kpz_window_reg_binding b
    where b.window_id = p_window_id
      and not exists (
          select 1
          from jsonb_array_elements(v_items) x
          where (x->>'reg_id')::int = b.reg_id
      );
end;
$$;
