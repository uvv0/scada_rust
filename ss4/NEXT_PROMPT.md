# Следующий промпт

```text
Продолжай работу в проекте `C:\andr\my2\ss4` как senior Rust performance engineer.

Текущее состояние:
- `cargo test` -> `89 passed, 0 failed, 4 ignored`
- `cargo fmt --check` -> успешно
- `cargo clippy --all-targets -- -D warnings` -> успешно
- `cargo build --release` -> успешно
- release-сборка чистая

Что уже оптимизировано:
1. Исправлен SQL bug с `obj.port::text`.
2. Убраны warnings и стабилизированы test/runtime paths.
3. Добавлены unit + ignored DB integration tests.
4. В worker runtime внедрен lazy copy-on-write для:
   - `rv`
   - `idx_seen`
   - `alarm_runtime`
5. `WorkerMerge` теперь не возвращает `idx_seen`/`alarm_runtime`, если dirty-флаги не выставлены.
6. `ScriptCache` получил per-`kpz` plan cache.
7. `flush_db_delta` переведен на column-based batch path без промежуточных tuple-векторов.
8. Очередь scheduler заменена на `JobQueue`:
   - global FIFO через `seq`
   - per-`kpz` pending
   - ready-set для spawnable jobs
9. Добавлены per-`kpz` кэши:
   - `idx_seen_by_kpz`
   - `alarm_runtime_by_kpz`
   - `primed_archive_once_by_kpz`
   - `relevant_reg_ids_by_kpz`
10. `build_worker_ctx_for_kpz` больше не делает полные проходы по этим runtime-структурам на каждый worker-start.
11. Script-mode и alarm post-hook больше не клонируют полный `Vec<RegBinding>` на каждую группу/задачу.

Цель следующей секции:
- Найти следующий реальный hot path или memory churn path.
- Не делать случайную “оптимизацию ради оптимизации”.
- Сначала провести локальный engineering-анализ hot spots по коду, затем сделать одну наиболее выгодную и локальную оптимизацию.

Приоритет анализа:
1. `build_worker_ctx_for_kpz` и состав `WorkerShared`
2. `run_a_mode` / `run_script_mode`
3. архивные пути:
   - `force_archive_once_reg_ids`
   - `primed_archive_once_kpz_reg`
4. построение request/block/group plan
5. batch DB write path
6. topology sync path
7. любые места с регулярным:
   - clone больших `HashMap/HashSet/Vec`
   - фильтрацией больших коллекций на каждый tick/job
   - повторным вычислением invariant-структур

Что нужно сделать:
1. Сначала коротко локализуй следующий bottleneck по коду.
2. Объясни, почему именно он сейчас следующий по приоритету.
3. Реализуй только одну основную оптимизацию за эту секцию.
4. Если нужно, добавь небольшой regression-test.
5. После правок обязательно прогоняй:
   - `cargo test`
   - `cargo build --release`

Ограничения:
- Не ломать текущую семантику scheduler.
- Не устраивать широкую архитектурную перепись без необходимости.
- Предпочитать локальные, проверяемые и обратимые изменения.
- Использовать `apply_patch` для ручных редактирований.
- Если появляются warnings, убрать их.
- Если для следующего шага уже нужен benchmark/profiler, явно скажи это.

В финале дай короткий отчет:
- какой path был выбран как следующий hot spot
- что было изменено
- какие лишние copy/filter/build операции убраны
- результаты `cargo test`
- результаты `cargo build --release`
- что теперь является следующим bottleneck по твоей оценке
```
