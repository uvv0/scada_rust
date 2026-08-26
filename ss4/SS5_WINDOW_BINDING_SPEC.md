# Спецификация привязок окон SS5 (черновик)

## Цель
Определить контракт привязки данных между окном UI SS5 и backend-моделью данных.

## Правила привязки
- Поля ввода сопоставляются со стабильными field ids, а не с отображаемыми labels.
- Числовые значения должны включать metadata единицы измерения и точности.
- Опциональные значения должны иметь явную семантику `null`.
- Действия записи должны быть idempotent и auditable.

## Валидация
- Required fields проверяются на client и server.
- Проверка диапазона выполняется для numeric controls.
- Enum values проверяются по backend dictionary.

## События
- `on_load`: получить начальное состояние.
- `on_change`: отметить dirty и проверить local state.
- `on_submit`: отправить нормализованный payload.
- `on_result`: показать результат success/error и обновить state.

## Обработка ошибок
- Transport error: retry с backoff.
- Validation error: показать field-level message.
- Conflict error: reload и показать diff перед повторной попыткой.
