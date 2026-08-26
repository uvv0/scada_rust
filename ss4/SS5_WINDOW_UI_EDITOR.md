# UI-редактор окон SS5: группы, привязки регистров и командные кнопки

Дата: 2026-02-18
Область: один экран редактора в `ss5` для выбранного `kpz`.

## Цель

Создать один визуальный редактор, где оператор может:
- выбирать группы для окна;
- привязывать регистры из выбранных групп к сетке окна;
- настраивать командные кнопки, которые отправляют действия.

## Макет экрана

1. Заголовок
- селектор `KPZ`;
- селектор `Window` (`code`, `title`);
- кнопки: `Save`, `Revert`, `Clone`, `Delete`.

2. Левая панель: `Groups + Available regs`
- список групп с multi-select;
- поиск/фильтр регистров по `id/name/addr/tip`;
- исходный список: только регистры из выбранных групп.

3. Центральная панель: `Window bindings`
- целевая сетка с колонками:
  - `pos`, `reg_id`, `name`, `addr`, `visible`, `writable`, `label`, `unit`, `fmt`
- drag/drop или move up/down;
- batch actions: show/hide, writable on/off, remove.

4. Правая панель: `Command buttons`
- список кнопок по порядку (`pos`);
- редактор кнопки: `code`, `title`, `style`, `confirm_text`, `active`;
- таблица actions для каждой кнопки:
  - `action_no`
  - `action_type` (`write_reg` или `post_cmd`)
  - для `write_reg`: `reg_id`, `value_num`
  - для `post_cmd`: `func(5/6)`, `addr_human`, `value_num`

## Объекты БД

Используются:
- `ui.kpz_window`
- `ui.kpz_window_group`
- `ui.kpz_window_reg_binding`
- `ui.kpz_window_command_button`
- `ui.kpz_window_command_action`
- `ui.group_window` — переиспользуемый group-level template window
- `ui.group_window_reg_binding` — регистры внутри group template
- `ui.kpz_window_group_window` — композиция одного KPZ window из group templates
- `ui.v_kpz_window_binding_resolved` — совместимая read model для resolved bindings

SQL-файлы:
- `ss5_window_binding_schema.sql`
- `ss5_window_commands_schema.sql`
- опциональный bulk helper: `ss5_window_binding_bulk.sql`
- `ss5_group_window_composition_schema.sql`

## Режим композиции: Group -> KPZ Window

- Runtime `GET /api/ss5/windows/{window_id}/bindings` может читать из `ui.v_kpz_window_binding_resolved`.
- Приоритет merge во view:
  - `legacy` row из `ui.kpz_window_reg_binding` имеет приоритет;
- composed `group` row из `ui.group_window_reg_binding`.
- Это позволяет постепенно мигрировать без поломки старых windows/API.

## Минимальный API

1. `GET /api/ss5/kpz/{kpz_id}/windows`
2. `POST /api/ss5/kpz/{kpz_id}/windows`
3. `GET /api/ss5/windows/{window_id}/groups`
4. `PUT /api/ss5/windows/{window_id}/groups` — full replace
5. `GET /api/ss5/windows/{window_id}/bindings`
6. `PUT /api/ss5/windows/{window_id}/bindings` — full replace
7. `GET /api/ss5/windows/{window_id}/buttons`
8. `PUT /api/ss5/windows/{window_id}/buttons` — full replace, включая actions
9. `GET /api/ss5/kpz/{kpz_id}/regs?groups=...`

## Правила валидации

- `window code` уникален внутри одного `kpz`
- `pos` уникален внутри каждого списка — groups, bindings, buttons
- `writable=true` только если ACL разрешает запись
- для `post_cmd`: `func in (5,6)`, `addr_human` обязателен
- для `write_reg`: `reg_id` обязателен

## Рекомендуемый порядок поставки

1. Применить SQL schemas.
2. Реализовать `GET/PUT groups` + `GET regs by groups`.
3. Реализовать `GET/PUT bindings`.
4. Реализовать `GET/PUT buttons/actions`.
5. Добавить UI editor в Windows app, затем переиспользовать тот же API в Web app.
