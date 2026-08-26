# ss7

Rust SCADA desktop application with egui-based UI and full configuration management.

## Features

- **Accounts** — user management with roles and permissions
- **IO Channels** — Modbus device configuration and tag mapping
- **KP Templates** — reusable device templates for quick setup
- **Scripts** — embedded scripting for automation logic
- **Windows** — HMI screen editor with drag-and-drop linking
- **Alarms** — alarm rules with acknowledgment and history
- **Dicts** — reference data dictionaries
- **Runtime** — live data monitoring and control

## Architecture

- `src/main.rs` — egui application entry point
- `src/app.rs` — main application state and actions
- `src/app/actions_*.rs` — action handlers for each subsystem
- `src/app_windows/` — window management (create, edit, delete, reload)
- `src/db*.rs` — per-domain database layers (core, accounts, alarms, config, dicts, regs, runtime, schema, UI windows)
- `src/modbus.rs` / `src/modbus_service.rs` — Modbus communication
- `src/models.rs` — domain data structures
- `src/theme.rs` — application visual theme
- `src/ui/` — egui window implementations

## UI Windows

- `accounts_window.rs` — user account management
- `script_editor_window.rs` — script editor with syntax highlighting
- `window_link_editor.rs` — visual HMI screen link editor
- `window_link_editor_preview.rs` — preview mode
- `window_link_editor_web_safe.rs` — web-safe export mode

## Configuration

SQLite database with schema defined in `src/db_schema.rs`. No external config files required.
