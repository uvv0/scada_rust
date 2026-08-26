# ss5 (Rust)

## Назначение
`ss5` — desktop инженерный инструмент для работы с KPZ/OBJ/REG, alarm и скриптами.

## Основные окна
- `KPZ editor` — редактирование KPZ параметров.
- `OBJ editor` — редактирование объекта связи.
- `REG editor` — редактирование регистров.
- `Refs editor` — справочники (`ip/port/speed/...`).
- `KPZ I/O` — ручной опрос/запись регистров.
- `Alarm rules` — правила и состояние тревог.
- `GScript editor` + `GScript output`.
- `ARX graph` — график по `arx_val`.

## UI-редактор в ss5
- Поддерживает KPZ window/bindings (`ui.kpz_window*`).
- Позволяет выбирать группы, добавлять регистры в bind и сохранять.
- `KPZ Preview` использует реальные координаты и размеры элементов (`x/y/w/h`) из `ui.kpz_window_reg_binding`.
- В preview подписи регистров рисуются сбоку от кнопок, как в `ss7`.
- Кнопка `Сосчитать` выполняет реальный пакетный Modbus-opros glued-блоками, близко к `ss7`.

## Актуальные правки
- В `REG editor` кнопка `New` не очищает поля (удобно для ввода похожих регистров).
- Генератор диапазона тестовых KPZ теперь создает записи с фиксированным `rtu=301`; изменяются только `id` и `modem`.
- В окне генератора диапазона добавлена явная подсказка про `rtu=301`.

## Сборка
```powershell
cargo check
cargo build --release
```

## Проверка проекта
Проверено 2026-07-13:
- `cargo check` проходит успешно.
- `cargo clippy --all-targets -- -D warnings` пока не проходит: найдено 42 предупреждения Clippy. Большая часть относится к механической чистке (`collapsible_if`, `manual_range_contains`, `identity_op`, `get_first`, лишние casts), но есть несколько мест, которые стоит посмотреть руками.

Рекомендуемый быстрый цикл перед правками:
```powershell
cargo fmt
cargo check
cargo clippy --all-targets
```

## Запуск
```powershell
.\target\release\ss5.exe
```

## Конфигурация БД
Приложение ищет `ss5.toml` рядом с `ss5.exe`. Пример есть в `ss5.toml.example`.

Приоритет настроек:
1. переменные окружения `PG_HOST`, `PG_PORT`, `PG_DB`, `PG_USER`, `PG_PASS`;
2. секция `[db]` в `ss5.toml`;
3. значения по умолчанию из `src/db.rs`.

Если основной host не `localhost` и подключение не удалось, `Db::connect_from_env()` пробует fallback на `localhost` с тем же портом, пользователем, паролем и именем БД.

## Runtime Scheduler Config (ss4)
- Added `Runtime cfg...` window in `ss5`.
- This window edits global scheduler parameters in DB table `public.scheduler_runtime_cfg`:
  - `no_response_failures`
  - `no_response_backoff_sec`
- These parameters are global for the whole `ss4` scheduler (not per-KPZ).

## GScript Templates
- Template storage:
  - `ui.gscript_template`: reusable PRE/POST templates and limits (`max_words`, `max_k`, `en`, `ver`, `elam`).
  - `ui.gscript_group_template`: binding `group_id -> template_id`.
  - Direct group script remains in `public.g_script`.
- Effective script priority:
  1. direct `public.g_script` for group;
  2. if missing, template from `ui.gscript_group_template`;
  3. otherwise script is absent.
- GScript editor actions:
  - `Load` loads direct group script;
  - `Load effective` loads script by effective priority above;
  - `Save` writes direct group script;
  - `Load tmpl` / `Save tmpl` / `Delete tmpl` manage templates;
  - `Bind->group` sets or clears template binding for group.

## Module Docs
- `MODULES_DOC_RU.md` - module map in Markdown.
- `MODULES_DOC_RU.html` - module map in HTML.

## Что улучшить дальше
- Разобрать Clippy backlog: сначала места с одинаковыми ветками в `src/app/windows/kpz_io.rs`, затем механические предупреждения по `collapsible_if` и диапазонам.
- Добавить минимальные unit-тесты для чистой логики: `src/modbus.rs`, `src/utils.rs`, `src/script.rs`. Сейчас сборка проверяется, но тестового контура в проекте почти нет.
- Вынести длинные DB-методы с большим числом аргументов (`update_kpz_full`, `update_obj`, `create_obj`, `update_reg_edit`) на входные DTO/patch-структуры. Это уменьшит риск перепутать поля при вызове.
- Разделить bootstrap/migration SQL из `Db::connect_from_env()` в отдельный модуль или SQL-файл. Сейчас приложение само создает/дополняет таблицы `ui.*` и `public.scheduler_runtime_cfg`, что удобно, но усложняет ревизию схемы.
- Убрать временные диагностические строки `[POST] ...` из `KPZ I/O Script` после финальной полевой проверки, если они больше не нужны оператору.
