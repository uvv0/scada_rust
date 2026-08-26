# Быстрый старт следующей секции Codex

Проект: `C:\andr\my2\ss4`
Дата снимка: 2026-04-30

## Текущее состояние
- Обычные тесты: `95 passed, 0 failed, 4 ignored`
- `cargo fmt --check`: успешно
- `cargo clippy --all-targets -- -D warnings`: успешно
- `cargo build --release`: успешно
- CI теперь запускает fmt + clippy + tests
- В CI есть явный job статуса DB-интеграции, поэтому отсутствие `TEST_DB_URL` видно как намеренный пропуск
- `ss4.toml` игнорируется Git; секреты не должны попадать в tracked-файлы
- `.env.example` документирует runtime-переменные окружения; реальные `.env*`-файлы игнорируются
- Порядок поиска конфигурации: каталог исполняемого файла, текущий рабочий каталог, затем переменные окружения
- Telegram-токены можно читать из env через `[telegram].bot_token_env` или стандартный `TELEGRAM_BOT_TOKEN`
- MQTT publisher можно включить через `[mqtt]`; он публикует `status`, `health`, `values/{kpz_id}` и `alarms/{kpz_id}/{rule_id}` через неблокирующую очередь
- Ключи `SCHED_AUTO_INFLIGHT*` оставлены только для совместимости; тесты проверяют, что они предупреждают/фиксируют наличие, но не меняют фиксированный `max_inflight`
- DB-интеграционные тесты остаются `#[ignore]` и требуют `TEST_DB_URL`
- UDP RX совместимость с ответами, где `DSR/MODEM` поменяны местами, реализована и покрыта тестами
- Fallback Script-mode RV для динамических ключей реализован (`rv_ctx` miss -> global RV cache)
- ELAM сохраняет summary для transport timeout/error
- DB fingerprint/topology SQL покрыт интеграционными тестами
- Декодирование DB-to-runtime `build_conn(...)` покрыто unit-тестами

## Недавние изменения безопасности планировщика
- Добавлен guard `KpzTask.generation` против устаревших worker merge после:
  - `start/stop`
  - restart-like reschedule
  - изменений topology на уровне task
- Добавлен guard `protocol_generation` против устаревших worker merge после перезагрузки protocol topology:
  - `regs`
  - `g_script`
  - `script_binding`
  - связанных protocol maps/caches
- Путь отбрасывания stale merge теперь логирует причину несовпадения:
  - `task_generation_mismatch`
  - `protocol_generation_mismatch`
  - `both`
- Добавлены regression-тесты для stale worker merge после:
  - stop
  - restart/backoff
  - task generation bump
  - protocol generation bump

## Недавняя локальная оптимизация
- `build_worker_ctx_for_kpz(...)` больше не создает временные one-item коллекции при каждом старте worker:
  - убран временный `HashMap` для `task`
  - убран временный one-item `HashSet` для `primed`
  - убран временный one-item `HashMap` для `last_a_glued_status`
- Worker context теперь передает эти значения напрямую:
  - `task`
  - `primed: bool`
  - `last_a_status: Option<String>`
- Script-mode и alarm post-hook больше не клонируют полный `Vec<RegBinding>` на каждую группу/задачу; они переиспользуют `Arc<Vec<RegBinding>>` и передают slices в `ScriptCache::get_plan`.

## Следующие приоритеты
1. Определить политику DB-интеграции в CI: optional secret-gated job или обязательная тестовая БД.
2. Продолжить анализ hot path внутри `run_a_mode` / `run_script_mode`.
3. Проверить archive-related churn при старте worker/runtime:
   - `force_archive_once_reg_ids`
   - `primed_archive_once_kpz_reg`
4. Проверить построение request/block/group plan на повторные вычисления в каждой задаче.
5. Если локального анализа кода будет недостаточно, перейти к benchmark/profiler-guided проходу.

## Быстрые команды
```powershell
cd C:\andr\my2\ss4
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build --release
$env:TEST_DB_URL = "postgresql://ss4_user:change-me@localhost:5432/ss4_db"
cargo test db_integration -- --ignored --nocapture
```
