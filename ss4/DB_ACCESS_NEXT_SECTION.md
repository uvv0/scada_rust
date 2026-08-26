# Заметки по доступу к БД

## Runtime
Для старта `ss4` нужны:
- `PG_HOST`
- `PG_PORT`
- `PG_DB`
- `PG_USER`
- `PG_PASS`

## Интеграционные тесты
По умолчанию игнорируются, включаются через:
- `TEST_DB_URL` — полный PostgreSQL URL

Пример:
```powershell
$env:TEST_DB_URL = "postgresql://ss4_user:change-me@localhost:5432/ss4_db"
cargo test db_integration -- --ignored --nocapture
```

Текущий локальный статус DB-интеграции:
- `4 passed, 0 failed`

Покрытые DB-specific проверки:
- alarm/arx roundtrip write path
- specific alarm rule path для `(kpz_id=5, reg_id=6002, rule_id=1)`, если правило есть
- загрузка topology fingerprint
- fingerprint aggregate `obj` с `port::text` на integer-колонке `port`

## CI
Job `db-integration` в `.github/workflows/ci.yml` запускается только когда настроен secret `TEST_DB_URL`.

Workflow также содержит `db-integration-status`, который запускается всегда и печатает, включены DB-интеграционные тесты или намеренно пропущены. Так статус CI остается явным даже без secret для тестовой БД.
