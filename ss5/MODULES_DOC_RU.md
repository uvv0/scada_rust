# Документация модулей `ss5`

Единая карта модулей и функций проекта `ss5` в формате, близком к `ss4`.

## Быстрый переход

- [Точка входа](#точка-входа)
- [Основные модули `src/`](#основные-модули-src)
- [Окна `src/app/windows/`](#окна-srcappwindows)
- [UI-модули `src/ui/`](#ui-модули-srcui)
- [Проверки и качество](#проверки-и-качество)
- [Что улучшить дальше](#что-улучшить-дальше)
- [Поток данных (кратко)](#поток-данных-кратко)

## Точка входа

- [`src/main.rs`](./src/main.rs)  
  Инициализация `egui/eframe`, установка panic hook, запуск `Ss5App`.

Ключевая функция:
- `main()` — собирает `Ss5App`, настраивает окно и стартует UI.

## Основные модули `src/`

### [`src/app.rs`](./src/app.rs)

Главный UI-контроллер приложения (экраны, редакторы, действия пользователя, графики, I/O, логи).

Ключевые функции:
- `Ss5App::try_new()` — стартовая инициализация состояния и первичная загрузка данных.
- `eframe::App::update(...)` — главный UI-цикл: автообновление, тулбар, окна, панели логов.
- `reload_all()`, `reload_logs()`, `reload_groups()`, `reload_kpz_refs()` — обновление runtime-данных.
- `save_kpz_meta()`, `save_kpz_full()`, `save_obj()`, `save_reg()`, `save_dict()` — сохранение редактируемых сущностей.
- `open_runtime_cfg_window()`, `save_runtime_cfg_to_db()` — управление runtime-конфигурацией `ss4`.
- `open_arx_state_window()`, `save_arx_state_row()` — просмотр/редактирование `arx_state`.
- `clear_arx_val_from_window()`, `clear_elam_from_window()`, `clear_poll_log_from_window()`, `clear_all_from_window()` — очистка runtime-таблиц из UI.
- `open_graph_window()` — открытие/перезагрузка ARX-графика.
- `open_kpz_io_window()`, `run_kpz_io_*` (внутренние обработчики) — ручной Modbus I/O.
- `open_gscript_editor()`, `eval_gscript_*` (внутренние обработчики) — PRE/POST редактор и прогон скрипта.

Важные helper-функции в модуле:
- `decode_pre_cmds(...)` — разбор PRE-команд чтения.
- `send_mb_over_udp(...)`, `parse_read_words_from_resp(...)`, `validate_modbus_response(...)` — низкоуровневый I/O для UI-инструментов.
- `format_elam_packet(...)`, `elam_expected_received(...)` — форматирование ELAM в UI.
- `ui_link_poll_now()` — реальный опрос `KPZ Preview` пакетными glued-запросами по блокам Modbus, близко к логике `ss7`.

### [`src/db.rs`](./src/db.rs)

PostgreSQL API для `ss5`: чтение/запись конфигурации, runtime-таблиц и UI-таблиц.

Функции подключения и базовой инициализации:
- `Db::connect_from_env()`

Конфигурация подключения:
- сначала читаются переменные окружения `PG_HOST`, `PG_PORT`, `PG_DB`, `PG_USER`, `PG_PASS`;
- затем `ss5.toml` рядом с exe, поддерживается секция `[db]` и старые top-level ключи `pg_host/pg_port/pg_db/pg_user/pg_pass`;
- если основной host не `localhost` и подключение не удалось, есть fallback на `localhost` с тем же портом и учетными данными.

Bootstrap схемы:
- `Db::connect_from_env()` при старте добавляет совместимые поля/таблицы через `create table if not exists` и `alter table if exists`;
- сейчас там создаются/дополняются `ui.kpz_window*`, `ui.gscript_template`, `ui.gscript_group_template`, `public.scheduler_runtime_cfg`;
- это удобно для запуска, но как точка улучшения SQL стоит вынести в отдельный migration-модуль или SQL-файл.

KPZ/OBJ/REG/справочники:
- `get_all_kpz()`, `update_kpz_meta()`, `update_kpz_full()`, `create_kpz_new()`
- `upsert_test_kpz_range()`, `set_kpz_start_range()`, `set_kpz_timing_range()`, `update_kpz_grups()`
- `get_all_obj()`, `update_obj()`
- `get_all_reg_edit()`, `update_reg_edit()`, `get_regs_for_group()`, `get_regs_by_groups()`
- `get_items()`, `upsert_item()`, `delete_item()`, `get_all_groups()`

Примечание по генератору диапазонов KPZ:
- `upsert_test_kpz_range()` теперь создает диапазон с фиксированным `rtu=301`.
- Для диапазона изменяются только `id` (`id_start..id_end`) и `modem` (`modem_start + offset`).

Логи и runtime-данные:
- `get_poll_log()`, `get_last_elam()`
- `get_last_arx_vals()`, `get_arx_series()`
- `get_arx_state_rows()`, `upsert_arx_state()`
- `clear_arx_val()`, `clear_elam()`, `clear_poll_log()`

GScript и шаблоны:
- `get_g_script()`, `upsert_g_script()`, `list_g_script_groups()`
- `list_g_script_templates()`, `upsert_g_script_template()`, `delete_g_script_template()`
- `get_group_template_id()`, `set_group_template()`, `get_effective_g_script()`

Scheduler runtime cfg:
- `get_scheduler_runtime_cfg()`, `upsert_scheduler_runtime_cfg()`

Alarm:
- `get_alarm_rules()`, `upsert_alarm_rule()`, `insert_alarm_rule()`, `delete_alarm_rule()`
- `get_alarm_state()`, `get_alarm_events()`

UI windows/bindings:
- `get_ui_kpz_windows()`, `upsert_ui_kpz_window()`
- `get_ui_window_groups()`, `save_ui_window_groups()`
- `get_ui_window_bindings()`, `save_ui_window_bindings()`
- `ui.kpz_window_reg_binding` в `ss5` теперь хранит геометрию preview-элемента: `x`, `y`, `w`, `h`.

### [`src/models.rs`](./src/models.rs)

DTO/модели строк БД и UI-структур:
- `KpzRow`, `ObjRow`, `RegRow`, `RegEditRow`, `GroupRow`, `DictItemRow`
- `PollLogRow`, `ElamRow`
- `ArxPointRow`, `ArxSeriesRow`, `ArxStateRow`
- `GScriptRow`, `GScriptTemplateRow`
- `AlarmRuleRow`, `AlarmStateRow`, `AlarmEventRow`
- `SchedulerRuntimeCfgRow`
- `UiKpzWindowRow`, `UiWindowGroupRow`, `UiWindowBindingRow`
- `UiWindowBindingRow` включает позицию/размер элемента preview: `x`, `y`, `w`, `h`.

### [`src/modbus.rs`](./src/modbus.rs)

Низкоуровневые Modbus/UDP утилиты.

Публичные функции:
- `crc16(data)`
- `shab(par, out_max_bytes)` — сборка UDP-заголовка.
- `sout_mb_only(...)` — сборка Modbus PDU/RTU без UDP-обертки.
- `sout(...)` — совместимая сборка полного пакета.
- `extract_modbus_frame(resp)` — выделение Modbus части из UDP ответа.
- `split_rx_to_virtual(rx)` — разделение multi-frame ответа.
- `build_mb_chunks(mb_frames, limit)` — чанкинг Modbus кадров по лимиту пакета.

### [`src/modbus_service.rs`](./src/modbus_service.rs)

Высокоуровневое чтение групп регистров через UDP/Modbus.

Публичные функции:
- `read_group_glued(conn, func, items, timeout)` — пакетное чтение группы, декодирование значений, возврат `tx/rx`.
- `request_reqs_glued(conn, reqs, timeout_per_chunk, idle_timeout)` — glued-опрос нескольких read-блоков с разбором multi-response и trace-строками.

Важные helper-функции:
- `send_mb_over_udp(...)`
- `parse_read_words_from_resp(...)`
- `send_chunk(...)`
- `next_packet_id()`
- `next_pid()`, `next_dsr()`
- `hex_join(...)`

### [`src/script.rs`](./src/script.rs)

Скриптовый DSL (PRE/POST): парсер, компиляция и VM-исполнение.

Публичные функции:
- `Script::parse(src)` — парсинг и компиляция скрипта.
- `Script::eval_result(...)` — выполнение с возвратом `regs/emits`, поддержка `print/print2`.

Ключевые внутренние блоки:
- parsing: `parse_stmt`, `parse_expr_*`, `parse_for_header_*`
- compile: `compile_stmts`, `compile_stmt`, `compile_expr`
- eval: `eval_bin`, `eval_call1/2/3`, `u16/i16/u32/i32/f32`, `dt2unix`, `bit`

### [`src/utils.rs`](./src/utils.rs)

Общие утилиты:
- `decode_groups(grups)` — байтовая маска -> список групп.
- `encode_groups(groups)` — список групп -> маска.
- `hex_full(data)` — hex форматирование бинарных буферов.

## Окна `src/app/windows/`

### [`src/app/windows/mod.rs`](./src/app/windows/mod.rs)

Подключает оконные подмодули к `Ss5App`.

### [`src/app/windows/alarm.rs`](./src/app/windows/alarm.rs)

Окно правил и состояния тревог:
- загрузка `alarm_rules`, `alarm_state`, `alarm_events`;
- подбор регистра по группе;
- создание, сохранение и удаление правил;
- валидация порогов `set_lo/set_lo_1/set_hi/set_hi_1`.

### [`src/app/windows/arx_state.rs`](./src/app/windows/arx_state.rs)

Окно runtime-состояния ARX:
- просмотр/редактирование `arx_state`;
- очистка `arx_val`, `elam`, `poll_log` по выбранному KPZ или без фильтра;
- пакетная очистка через `clear_all_from_window()`.

### [`src/app/windows/arx_val.rs`](./src/app/windows/arx_val.rs)

Просмотр последних строк `arx_val` с фильтром по выбранному KPZ.

### [`src/app/windows/dict_editor.rs`](./src/app/windows/dict_editor.rs)

Редактор справочников:
- загрузка элементов выбранной таблицы;
- синхронизация формы с выбранной строкой;
- upsert/delete элемента.

### [`src/app/windows/graph.rs`](./src/app/windows/graph.rs)

Окно графика:
- выбор группы и регистра;
- загрузка серии через `get_arx_series()`;
- отрисовка `egui_plot` с форматированием времени.

### [`src/app/windows/group_editor.rs`](./src/app/windows/group_editor.rs)

Редактор маски групп KPZ:
- ввод списка групп;
- проверка диапазона `1..=512`;
- сохранение через `update_kpz_grups()`.

### [`src/app/windows/gscript.rs`](./src/app/windows/gscript.rs)

Редактор и вывод GScript:
- вкладки PRE/POST;
- `Load`, `Load effective`, `Save`;
- управление шаблонами `ui.gscript_template`;
- привязка шаблона к группе через `ui.gscript_group_template`;
- запуск/валидация скрипта и просмотр `print`, `regs`, `emits`.

### [`src/app/windows/kpz_editor.rs`](./src/app/windows/kpz_editor.rs)

Полный редактор KPZ:
- загрузка формы из выбранного KPZ;
- сохранение полного набора параметров;
- создание новой KPZ-записи.

### [`src/app/windows/kpz_io.rs`](./src/app/windows/kpz_io.rs)

Ручной Modbus/KPZ I/O:
- чтение Input/Holding;
- запись Holding;
- TU-команды через функцию 5;
- запуск PRE/POST GScript по выбранной группе;
- fallback `rv(...)` на последние `arx_val`;
- запись результатов `reg(...)` и `emit(...)` обратно в `reg.val`.

Заметка: сейчас в `Script log` остаются диагностические строки `[POST] result ...` и `[POST] emit ...`. Их можно убрать после финальной проверки, если оператору они не нужны.

### [`src/app/windows/obj_editor.rs`](./src/app/windows/obj_editor.rs)

Редактор объекта связи:
- выбор IP/port/speed/format из справочников;
- сохранение существующего объекта;
- создание нового объекта.

### [`src/app/windows/range_kpz.rs`](./src/app/windows/range_kpz.rs)

Окно массовых операций над диапазоном KPZ:
- создание/обновление тестового диапазона;
- включение/выключение `start`;
- настройка timing;
- массовое включение/выключение групп.

### [`src/app/windows/reg_editor.rs`](./src/app/windows/reg_editor.rs)

Редактор регистров:
- фильтрация по группе;
- выбор справочников `n_reg`, `n_p`, `n_s`;
- сохранение/создание регистра;
- `New` намеренно не очищает поля, чтобы быстрее вводить похожие регистры.

### [`src/app/windows/runtime_cfg.rs`](./src/app/windows/runtime_cfg.rs)

Окно глобальной runtime-конфигурации планировщика `ss4`:
- загрузка `public.scheduler_runtime_cfg`;
- сохранение backoff/timeout/metrics параметров.

## UI-модули `src/ui/`

### [`src/ui/mod.rs`](./src/ui/mod.rs)

Точка подключения UI-подмодулей.

### [`src/ui/window_link_editor.rs`](./src/ui/window_link_editor.rs)

UI-редактор привязок окон/регистров (`ui.kpz_window*`) и предпросмотра.

Публичная функция:
- `show_ui_link_editor(...)` — рисует окно редактора и возвращает действия (`UiLinkEditorAction`).

Важные helper-функции:
- `fmt_live(...)`
- `binding_rect(...)`
- `is_tu_binding(...)`, `is_bool_binding(...)`, `is_word16_binding(...)`
- `preview_edit_seed(...)`

Заметки по текущему поведению preview:
- Preview рисуется по реальным координатам/размерам из БД (`x/y/w/h`), а не по синтетической сетке.
- Подпись регистра отображается сбоку от кнопки, как в `ss7`.
- Кнопка `Сосчитать` выполняет реальный glued-опрос Modbus-блоков для видимых bindings.

## Поток данных (кратко)

1. `main` запускает `Ss5App`.
2. `app` загружает данные через `db` и строит интерфейс.
3. Операции I/O и диагностика идут через `modbus_service` и `modbus`.
4. Скриптовые вычисления PRE/POST выполняются через `script`.
5. Изменения сохраняются обратно в БД через `db`.

## Проверки и качество

Проверено 2026-07-13:
- `cargo check` проходит успешно.
- `cargo clippy --all-targets -- -D warnings` не проходит: найдено 42 предупреждения.

Основные группы Clippy-предупреждений:
- механические упрощения: `collapsible_if`, `manual_range_contains`, `identity_op`, `get_first`, лишние casts;
- `too_many_arguments` у DB-методов: `update_kpz_full`, `update_obj`, `create_obj`, `update_reg_edit`;
- `if_same_then_else` в `src/app/windows/kpz_io.rs` при выборе target-регистра для POST `reg(...)` и `emit(...)`;
- `derivable_impls` для `UiLinkEditorState`.

Рекомендуемый цикл проверки:
```powershell
cargo fmt
cargo check
cargo clippy --all-targets
```

## Что улучшить дальше

1. Сначала разобрать не только стиль, но и смысловые Clippy-сигналы:
   - одинаковые ветки в `kpz_io.rs`;
   - слишком широкие DB-методы с большим числом аргументов;
   - ручной `Default` там, где можно `#[derive(Default)]`.
2. Добавить unit-тесты для чистой логики:
   - Modbus CRC/кадры/чанкинг в `src/modbus.rs`;
   - кодирование групп в `src/utils.rs`;
   - парсер и VM GScript в `src/script.rs`.
3. Вынести bootstrap SQL из `Db::connect_from_env()` в отдельный migration-модуль или SQL-файл.
4. Документировать схему БД отдельно: `public.kpz/obj/reg`, runtime-таблицы, `ui.*`.
5. После полевой проверки убрать временные диагностические `[POST] ...` строки из `KPZ I/O Script`, если они больше не нужны.
