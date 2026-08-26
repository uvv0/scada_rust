# Реестр рисков (ss4)

Дата: 2026-04-30
Проект: `C:\andr\my2\ss4`

## Шкала
- Критичность: `Critical` / `High` / `Medium` / `Low`
- Статус: `Open` / `Mitigating` / `Monitoring` / `Closed`

## Риски
| ID | Риск | Критичность | Влияние | Митигация | Статус |
|---|---|---|---|---|---|
| R1 | DB-интеграционные тесты пропускаются в CI, когда отсутствует DB URL | High | Ложный зеленый статус для DB path | Отдельный CI job + явный secret `TEST_DB_URL` | Mitigating |
| R2 | Большой `scheduler.rs` повышает риск регрессий | High | Сложно анализировать и review | Разделение на `queue/worker/merge/metrics`; `db_writer.rs`, `db_delta.rs`, `constants.rs`; типы очереди в `queue.rs`; типы worker в `worker.rs`; `ReadBlock` в `poll_plan.rs`; `GroupPlan`/`BlockPlan` в `amode.rs`; `PreCmd`/`decode_pre_cmds` в `smode.rs` | Mitigating |
| R3 | Краевые случаи UDP-корреляции под нагрузкой | High | Неверное сопоставление ответов в экстремальных/потерянных сетевых условиях | Корреляция по `(ip,port,pid,dsr)`, очистка pending при timeout, отбрасывание modem mismatch, совместимость swapped DSR/MODEM, stress tests | Mitigating |
| R4 | Clone-heavy merge paths в планировщике | Medium | CPU/RAM overhead на масштабе | Benchmark и оптимизация data sharing | Open |
| R5 | Слишком подробное логирование на hot path | Medium | Throughput и I/O overhead | Добавить sampling/rate limits для debug paths | Monitoring |
| R6 | Локальные секреты случайно попадают в commit | Critical | Утечка token/DB credentials | `ss4.toml` игнорируется; в Git хранить только примеры; leaked tokens ротировать | Mitigating |
| R7 | Разный поиск конфигурации между `cargo run` и release exe | Medium | Локальный запуск неожиданно использует env или не стартует | Поиск `ss4.toml` рядом с exe, затем в текущем рабочем каталоге, затем env | Closed |

## Текущие приоритеты
1. Финализировать политику DB-интеграции в CI: обязательный gate или optional job.
2. ~~Декомпозиция scheduler (R2):~~ выполнено — `scheduler.rs` ~260 строк; вынесены queue/worker/db_delta/db_writer/constants/amode/smode/poll_plan; обычный набор 89 passed.
3. ~~UDP stress tests:~~ выполнено — late response, reordered responses, duplicate response; modem/DSR покрытие в обычном наборе.
4. R4: benchmark clone-heavy merge paths; R5: sampling/rate limits на hot-path logging.
5. R6: ротировать локальный Telegram token, если он когда-либо публиковался вне этого workspace.
