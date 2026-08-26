# Руководство по переменным планировщика и памяти (ss4)

Дата: 2026-03-05
Проект: `C:\andr\my2\ss4`

## Основные переменные
1. `SCHED_POOL_SIZE`
- Значение: жесткая верхняя граница worker concurrency внутри процесса.
- Влияние на память: большее значение допускает больше одновременных worker states и buffers, увеличивая пиковое потребление RAM.

2. `SCHED_MAX_INFLIGHT`
- Значение: базовое число concurrent jobs.
- Влияние на память: прямое увеличение active-job memory footprint при росте значения.

3. `SCHED_AUTO_INFLIGHT`
- Статус: только совместимость; принимается парсером конфигурации, но не реализовано в runtime.
- Текущее поведение: при наличии `ss4` пишет warning и продолжает использовать фиксированный `SCHED_MAX_INFLIGHT`.
- Влияние на память: отсутствует сверх фиксированного значения `SCHED_MAX_INFLIGHT`.

4. `SCHED_AUTO_INFLIGHT_MAX`
- Статус: только совместимость; принимается, но не применяется.
- Влияние на память: отсутствует до реализации dynamic inflight.

5. `SCHED_AUTO_INFLIGHT_BACKLOG_PER_SLOT`
- Статус: только совместимость; принимается, но не применяется.
- Влияние на память: отсутствует до реализации dynamic inflight.

6. `SCHED_MAX_QUEUE`
- Значение: максимальная длина внутренней очереди.
- Влияние на память: прямой лимит backlog memory; большая очередь допускает больше queued jobs в RAM.

## Практический порядок настройки памяти
1. Уменьшить `SCHED_MAX_INFLIGHT`.
2. Уменьшить `SCHED_POOL_SIZE`, если он намного выше `SCHED_MAX_INFLIGHT`.
3. Уменьшить `SCHED_MAX_QUEUE`.
4. Держать `SCHED_MAX_INFLIGHT` близко к расчетному минимуму по throughput.

## Рекомендуемый профиль: 40 сек, 1000 мс
```powershell
$env:SCHED_POOL_SIZE="420"
$env:SCHED_MAX_INFLIGHT="280"
$env:SCHED_MAX_QUEUE="8000"
```

## Рекомендуемый профиль: 25 сек, 1000 мс
```powershell
$env:SCHED_POOL_SIZE="600"
$env:SCHED_MAX_INFLIGHT="420"
$env:SCHED_MAX_QUEUE="10000"
```

## Заметки
1. Память очереди обычно меньше runtime caches (`rv_by_kpz`, script bindings, alarm state), но лимиты очереди все равно важны для пиковой стабильности.
2. Если timeout rate растет после увеличения inflight, сначала уменьшайте `SCHED_MAX_INFLIGHT`.
3. Ключи `SCHED_AUTO_INFLIGHT*` зарезервированы для будущей реализации dynamic inflight. Не опирайтесь на них при текущей настройке.
