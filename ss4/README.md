# ss4 (Rust)

## Назначение
`ss4` — backend-сервис опроса устройств КПЗ без пользовательского интерфейса.

Сервис:
- загружает конфигурацию опроса из PostgreSQL;
- выполняет циклы A-mode и Script-mode;
- пишет значения в `arx_val`, состояние в `arx_state`;
- пишет трассу обмена и журналы в `poll_log` и `elam`.

## Режим опроса
- Текущий режим: `parallel worker queue` — фиксированный пул воркеров и общая очередь.
- Задачи A-mode и Script-mode ставятся в очередь по времени запуска и выполняются параллельно до лимита `max_inflight`.
- Сбой одного воркера КПЗ не останавливает обработку остальных задач КПЗ.
- Для одного и того же `kpz_id` планировщик запускает только одну задачу одновременно.

## Безопасность UDP-корреляции
- Ключ сопоставления запроса и ответа: `(ip, port, packet_id, dsr)`.
- При таймауте состояние ожидающего запроса удаляется; поздние пакеты для этого запроса отбрасываются.
- Для полных заголовков проверяется значение `modem`; ответы с несовпадающим `modem` отбрасываются.
- Поддерживается совместимость RX с ответами, где поля `DSR/MODEM` поменяны местами.
- Повторное использование PID защищено TTL в аллокаторе (`next_pid`), что снижает риск коллизии со старыми пакетами.

## Основные модули
- `src/scheduler.rs`: оркестрация планировщика и главный цикл (~260 строк); подробная карта модулей — в `MODULES_DOC_RU.md`.
- `src/scheduler/`: `db_delta`, `db_writer`, `queue`, `worker`, `merge`, `metrics`, `constants`, `amode`, `smode`, `poll_plan`, `alarm`, `state_sync`, `rv_state`, `post_cmd`, `support`.
- `src/modbus_service.rs`: пакетирование Modbus-запросов поверх UDP.
- `src/modbus.rs`: низкоуровневые помощники Modbus/CRC/HDR.
- `src/script.rs`: парсер и исполнитель MiniScript.
- `src/script_cache.rs`: кэш скомпилированных скриптов.
- `src/db_queries.rs`: операции загрузки и записи БД.

## Запуск
Создайте `ss4.toml` рядом с исполняемым файлом или в текущем рабочем каталоге по примеру ниже:
```toml
[db]
host = "localhost"
port = 5432
db = "ss4_db"
user = "ss4_user"
pass = "change-me"

[scheduler]
pool_size = 600
tick_ms = 250
sync_period_sec = 2
max_queue = 10000
max_inflight = 420
```

Порядок поиска конфигурации:
1. `ss4.toml` рядом с исполняемым файлом.
2. `ss4.toml` в текущем рабочем каталоге.
3. Если файл не найден, используются переменные окружения `PG_HOST/PG_PORT/PG_DB/PG_USER/PG_PASS`.

Используйте `.env.example` как чек-лист переменных окружения. Реальные `.env`-файлы должны оставаться локальными; Git их игнорирует.

Runtime-политика отсутствия ответа загружается из таблицы БД `scheduler_runtime_cfg`:
- `no_response_failures` — по умолчанию `3`;
- `no_response_backoff_sec` — по умолчанию `600`.

`ss4` также автоматически создает эту таблицу при старте, если ее нет, и гарантирует наличие строки `id=1`.

Конфигурация планировщика читается из секции `[scheduler]` в `ss4.toml`.
Для отсутствующих полей используется fallback на переменные окружения:
- `SCHED_POOL_SIZE` — по умолчанию `420`;
- `SCHED_TICK_MS` — по умолчанию `250`;
- `SCHED_SYNC_PERIOD_SEC` — по умолчанию `2`;
- `SCHED_MAX_QUEUE` — по умолчанию `10000`;
- `SCHED_MAX_INFLIGHT` — по умолчанию значение `SCHED_POOL_SIZE`.

Примечание по совместимости: ключи и переменные `auto_inflight*` принимаются только для обратной совместимости парсинга конфигурации. Текущий runtime пишет предупреждение при их наличии и все равно использует фиксированный `max_inflight`.

Telegram-уведомления об авариях опциональны. Предпочтительно хранить токен бота в переменной окружения, а не в `ss4.toml`:
```toml
[telegram]
enabled = true
bot_token_env = "TELEGRAM_BOT_TOKEN"
queue_cap = 200
```

Если `bot_token_env` не задан, используется `TELEGRAM_BOT_TOKEN`. Поле `bot_token` в файле все еще поддерживается как fallback для совместимости, но его не стоит использовать для реальных секретов.

MQTT-публикация во внешние системы опциональна. Первый MVP публикует только outbound-события: values, alarms, health/status. Команды через MQTT и Sparkplug B пока не включены.
```toml
[mqtt]
enabled = true
host = "127.0.0.1"
port = 1883
client_id = "ss4"
username_env = "MQTT_USER"
password_env = "MQTT_PASS"
topic_prefix = "ss4/v1"
queue_cap = 1000
qos = 1
retain_health = true
publish_values = true
publish_alarms = true
# value_kpz_ids = [3]
# value_group_ids = [21]
# value_reg_ids = [6001, 6002]
```

Topics:
- `ss4/v1/status` — retained `online`, LWT `offline`;
- `ss4/v1/health` — retained JSON health snapshot;
- `ss4/v1/values/{kpz_id}` — JSON batch значений;
- `ss4/v1/alarms/{kpz_id}/{rule_id}` — JSON alarm event.

Пример payload для values:
```json
{
  "ts": 1777561200,
  "kpz_id": 1000,
  "kpz_name": "КПЗ 1000",
  "values": [
    {
      "reg_id": 6001,
      "addr": 30401,
      "name": "pressure",
      "group_id": 3,
      "tip": 1,
      "value": 12.3,
      "quality": "ok"
    }
  ]
}
```

MQTT отправляется через ограниченную неблокирующую очередь. Если broker недоступен или очередь переполнена, scheduler продолжает опрос, а событие MQTT может быть отброшено с warning в log.

Для снижения нагрузки можно ограничить поток значений массивами `value_kpz_ids`, `value_group_ids`, `value_reg_ids`. Пустые или отсутствующие массивы означают “публиковать все”. Если задано несколько фильтров, они применяются вместе: значение должно пройти каждый из них.

Пошаговая проверка с Mosquitto/MQTTX/Node-RED описана в `MQTT_QUICKSTART.md`.

Защита `poll_log` в деградированных режимах и при сбоях:
- health-записи (`health_ok/warn/crit`) ограничены по частоте — не чаще одного раза в 60 секунд на тип;
- `poll_log` очищается батчами по retention-периоду, по умолчанию 14 дней, аналогично очистке `elam`.

Отдельный schema-файл для `scheduler_runtime_cfg` не нужен: `ss4` создает таблицу при старте, если она отсутствует.

Запуск из корня репозитория:
```powershell
cargo run --release
```

Или запуск release-исполняемого файла напрямую:
```powershell
.\target\release\ss4.exe
```

## Тесты
- Обычный набор — unit + non-DB: `cargo test`
- DB-интеграционные тесты, по умолчанию игнорируются:
```powershell
$env:TEST_DB_URL = "postgresql://ss4_user:change-me@localhost:5432/ss4_db"
cargo test db_integration -- --ignored --nocapture
```

DB-интеграционное покрытие включает:
- `db_queries::tests::db_integration_alarm_and_arx_val_roundtrip`
- `db_queries::tests::db_integration_specific_rule_kpz5_reg6002_rule1`
- `db_queries::tests::db_integration_obj_fingerprint_query_accepts_integer_port_column`
- `db_queries::tests::db_integration_load_topology_fingerprint_succeeds`

Текущий локальный результат:
- обычный набор: **95 passed**, 0 failed, 4 ignored;
- DB-интеграционный набор с `TEST_DB_URL`, указывающим на подготовленную локальную PostgreSQL БД: 4 passed, 0 failed.

Заметные группы тестов:
- UDP transport: timeout/cleanup, reordered responses, late response dropped, duplicate response, swapped DSR/MODEM, modem mismatch.
- Scheduler: queue (retain, pop_next_spawnable), DbDelta (is_empty, total_rows, drop_poll_logs, coalesce), decode_pre_cmds (smode), регрессии worker/merge.
- Modbus: расширенное RTU-кодирование (248..1997), границы протокола.

## Последние изменения
- Добавлена protocol-side RTU-валидация в сборщике Modbus-пакетов:
  - поддерживаемый wire-диапазон: `1..=1997`;
  - расширенное RTU-кодирование использует `F8..FE` + `00..249`;
  - RTU вне диапазона отклоняется до UDP-отправки.
- UDP-проверка `modem` в RX теперь совместима с ответами некоторых устройств, где поля `DSR/MODEM` поменяны местами.
- Добавлены подробные debug-поля для трассировки транспорта: TX/RX modem, expected/got modem, DSR.
- При transport timeout/errors `ss4` пишет summary-строки ELAM (`ERROR: transport: timeout`), а не оставляет ELAM пустым.
- Script-mode `rv()` теперь использует fallback на глобальный runtime RV cache, если ключ не найден в заранее собранном script context. Это важно для динамических ключей вроде `base + 20` / `base + 21`.
- Успешные Script-mode задачи теперь сбрасывают накопленную серию `no_response`, как A-mode, и не провоцируют ложный backoff после единичных script timeout.
- Script-mode теперь сохраняет summary-строки ELAM до возврата ошибки `responses count mismatch`, поэтому частичные glued-response инциденты видны в диагностике БД.
- Добавлено unit-покрытие для декодирования DB-to-runtime `build_conn(...)`, включая lookup-id и invalid-input paths.
- Добавлено DB-интеграционное покрытие для topology fingerprint SQL и агрегата `obj.port::text`.
- Добавлен transport compatibility test:
  - `udp_transport::tests::send_accepts_response_with_swapped_dsr_modem_fields`
- Добавлены regression-тесты планировщика:
  - `scheduler::tests_async::run_script_job_success_clears_no_response_streak`
  - `scheduler::tests_async::run_script_mode_partial_response_persists_elam_summary_before_error`
- **Декомпозиция R2:** `scheduler.rs` сокращен до ~260 строк; вынесены `db_delta`, `db_writer`, `constants`, типы очереди в `queue.rs`, типы worker/merge в `worker.rs`, `ReadBlock` в `poll_plan`, `GroupPlan`/`BlockPlan` в `amode`, `PreCmd`/`decode_pre_cmds` в `smode`. См. `RISK_REGISTER.md` и `MODULES_DOC_RU.md`.
- **Тесты:** UDP stress (late response, reordered responses, duplicate response); исправлены тесты расширенного Modbus RTU для валидного диапазона 248..1997; queue (`job_queue_retain`, `job_queue_one_kpz_not_parallel`); DbDelta (`is_empty`, `total_rows`, `drop_poll_logs`); `decode_pre_cmds` (smode); Telegram token resolution; MQTT config/payload/topic tests; совместимость fixed `max_inflight`. Обычный набор: 95 passed.
- **MQTT MVP:** добавлена опциональная публикация `status`, `health`, `values/{kpz_id}`, `alarms/{kpz_id}/{rule_id}` через неблокирующую очередь.
- **Запуск:** release-файл `target\release\ss4.exe`; конфигурация через `ss4.toml` рядом с exe или через переменные окружения.

Операционная заметка по локальному запуску от 11 марта 2026:
- После нормализации `kpz.rtu = 301`, обновления генерации диапазона `ss5` и пересборки `im1` прогон по `kpz_id 1000..7000` завершился чисто.
- В финальном окне запуска `elam` содержал только строки `OK`.
- Health-снимки `poll_log` показывали `started=10000, ok=10000, err=0, timeout=0`.

## CI
Workflow: `.github/workflows/ci.yml`
- `tests`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, build + обычный прогон тестов + release build.
- `db-integration`: запускает ignored DB-тесты, когда задан secret `TEST_DB_URL`.
- `db-integration-status`: всегда сообщает, включены DB-интеграционные тесты или намеренно пропущены.
