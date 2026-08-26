# Документация модулей `ss4`

Единая карта модулей проекта с быстрыми ссылками.

## Быстрый переход

- [Точка входа](#точка-входа)
- [Основные модули `src/`](#основные-модули-src)
- [Подмодули планировщика `src/scheduler/`](#подмодули-планировщика-srcscheduler)
- [Тесты планировщика](#тесты-планировщика)
- [Поток данных (кратко)](#поток-данных-кратко)

## Точка входа

- [`src/main.rs`](./src/main.rs)
  Запуск сервиса, чтение конфигурации, подключение к БД, старт scheduler runtime.

## Основные модули `src/`

- [`src/scheduler.rs`](./src/scheduler.rs)
  Оркестратор runtime-цикла и состав core-типов планировщика.

- [`src/db.rs`](./src/db.rs)
  Подключение к PostgreSQL и первичная инициализация `scheduler_runtime_cfg`.

- [`src/db_queries.rs`](./src/db_queries.rs)
  SQL API для загрузки конфигурации и записи runtime-данных (`arx_val`, `poll_log`, `elam`, alarms).

- [`src/types.rs`](./src/types.rs)
  Доменные структуры (`KpzRow`, `ObjRow`, `ConnInfo`, `AlarmRule`, `ScriptBindingRow` и др.).

- [`src/modbus.rs`](./src/modbus.rs)
  Низкоуровневые Modbus/CRC/кадры.

- [`src/modbus_service.rs`](./src/modbus_service.rs)
  Высокоуровневый glued-обмен Modbus-запросами.

- [`src/udp_transport.rs`](./src/udp_transport.rs)
  Коррелированный UDP request/response transport.

- [`src/script.rs`](./src/script.rs)
  Движок DSL скриптов PRE/POST.

- [`src/script_cache.rs`](./src/script_cache.rs)
  Кэш шаблонов и resolved plan для script-mode.

## Подмодули планировщика `src/scheduler/`

- [`src/scheduler/amode.rs`](./src/scheduler/amode.rs)
  Типы `GroupPlan`, `BlockPlan` и A-mode цикл опроса.

- [`src/scheduler/smode.rs`](./src/scheduler/smode.rs)
  Script-mode цикл (PRE/POST/bindings/write-back); тип `PreCmd` и `decode_pre_cmds`.

- [`src/scheduler/worker.rs`](./src/scheduler/worker.rs)
  Типы воркера и merge (`WorkerCtx`, `WorkerMerge`, `WorkerRuntimeDelta`, `IdxSeen`, `AlarmRuntime` и др.), выполнение job и упаковка merge-результата.

- [`src/scheduler/queue.rs`](./src/scheduler/queue.rs)
  Типы очереди заданий (`Job`, `JobKind`, `JobQueue`, `KpzTask`) и диспетчеризация due-задач с backpressure.

- [`src/scheduler/db_delta.rs`](./src/scheduler/db_delta.rs)
  Типы батчевой записи в БД: `DbDelta`, `PollLogRow`, `AlarmStateUpdate`, `AlarmEventRow`, `ArxStateUpdate` и их coalescing.

- [`src/scheduler/db_writer.rs`](./src/scheduler/db_writer.rs)
  Асинхронная запись батчей `DbDelta` в БД: очередь + coalescing + backpressure shed.

- [`src/scheduler/constants.rs`](./src/scheduler/constants.rs)
  Константы планировщика: retention, метрики, аварии (`RV_ALARM_*`), post-cmd ключи (`POST_CMD_*`).

- [`src/scheduler/merge.rs`](./src/scheduler/merge.rs)
  Слияние worker-результатов в общий state.

- [`src/scheduler/metrics.rs`](./src/scheduler/metrics.rs)
  Runtime-метрики и health-оценка.

- [`src/scheduler/state_sync.rs`](./src/scheduler/state_sync.rs)
  Декомпозированный sync с БД:
  `load_sync_rows -> reload_topology_from_rows -> reload_alarm_state -> run_retention_cleanups`.

- [`src/scheduler/rv_state.rs`](./src/scheduler/rv_state.rs)
  RV/IDX state, quality и mapping регистров.

- [`src/scheduler/poll_plan.rs`](./src/scheduler/poll_plan.rs)
  Тип `ReadBlock` и планирование групп/блоков опроса (`plan_group_reads`, `build_blocks_with_func`).

- [`src/scheduler/alarm.rs`](./src/scheduler/alarm.rs)
  Вычисление alarm-правил и запись alarm-state/event.

- [`src/scheduler/post_cmd.rs`](./src/scheduler/post_cmd.rs)
  Декодирование и построение post-команд устройства.

- [`src/scheduler/support.rs`](./src/scheduler/support.rs)
  Общие утилиты scheduler, включая единый transport helper `exec_glued_reqs`.

## Тесты планировщика

- [`src/scheduler/tests_core.rs`](./src/scheduler/tests_core.rs)
  Unit-тесты scheduler: очередь (retain, pop_next_spawnable), DbDelta (is_empty, total_rows, drop_poll_logs), регрессии worker/merge.

- [`src/scheduler/tests_async.rs`](./src/scheduler/tests_async.rs)
  Async/интеграционные тесты (`tokio::test`) worker/A-mode/Script-mode путей.
  Включая регрессии:
  `run_script_job_success_clears_no_response_streak` и
  `run_script_mode_partial_response_persists_elam_summary_before_error`.

## Поток данных (кратко)

1. `main` инициализирует БД и scheduler.
2. `state_sync` загружает конфигурацию из БД и обновляет runtime state.
3. `queue` ставит due A/Script jobs в очередь.
4. `worker` запускает `amode`/`smode`; transport выполняется через `modbus_service + udp_transport`.
5. `merge` сливает результаты; `metrics` формирует health; данные пишутся в `arx_val/poll_log/elam/alarm`.
