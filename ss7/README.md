# ss7 (Rust)

Desktop UI designer for KPZ windows/bindings with live preview.

## Stack
- `eframe/egui`
- `tokio-postgres`
- Modbus-over-UDP helpers for preview polling and write commands

## Structure
- `src/app.rs` - main app state, initialization and UI loop dispatch.
- `src/app/` - app feature modules:
  - `background.rs` - background worker result types and polling
  - `actions_io.rs` - Modbus preview polling and write commands
  - `actions_windows.rs`, `actions_templates.rs`, `actions_kp_templates.rs`, `actions_accounts.rs` - UI actions by feature area
  - `app_support.rs` - shared app helpers
- `src/app_windows/` - worker result types and helpers for window-related DB/UI operations
- `src/ui/window_link_editor.rs` - UI Link Editor (Groups / Available regs / Bindings), preview and layout tools
- `src/db.rs` - DB entry point and connection/migration facade
- `src/db_*.rs` - DB methods split by concern: accounts, dicts, regs, UI windows, KP templates, alarms, runtime/history, core CRUD
- `src/modbus_service.rs`, `src/models.rs` - Modbus service and DTOs

## Run
Configure DB by either:
1. env vars: `PG_HOST`, `PG_PORT`, `PG_DB`, `PG_USER`, `PG_PASS`
2. `ss7.toml` near project/exe

Then:
```powershell
cargo run
```

## Schema Migrations
Schema migrations are executed automatically during DB initialization in `Db::connect_from_env()`.

You can also apply them manually:
```powershell
$env:PG_USER="your_user"
$env:PG_PASS="your_pass"
.\apply_ss7_schema.ps1
```

SQL source: `ss7_ui_schema.sql`

## Tests
Default:
```powershell
cargo test
```

DB integration (ignored by default, requires `TEST_DB_URL`):
```powershell
$env:TEST_DB_URL="postgresql://user:pass@host:5432/dbname"
cargo test db_integration -- --ignored --nocapture
```

## Notes
- `Reload refs` runs in a background worker channel to avoid UI freeze on DB reads.
- Main UI modes include window templates, KP template sets, KP binding and KP window preview workflows.
- `UI Link Editor` uses a `Groups / Available regs / Bindings` layout and supports component kinds such as `auto`, `led`, `numeric`, `bar`, `gauge`, `setpoint`, and `button`.
- `Save window` persists the current window without clearing Code, Title or Description fields after save.
- Web-account passwords are now stored with `argon2id`; projects that authenticate against `public.web_users` should support `argon2` hashes.
