# ss4

Rust SCADA server application with modular architecture.

## Features

- **Modbus TCP/RTU** — poller with configurable intervals, delta writes, alarm rules
- **Scheduler** — time-based polling plans, script bindings, post-command execution
- **Script engine** — embedded Lua VM for custom logic and automation
- **PostgreSQL** — persistent storage for configuration, runtime data, and archives
- **MQTT** — publish/subscribe integration for external systems
- **Web UI** — Qt-based operator interface with window composition editor

## Architecture

- `src/main.rs` — entry point, config loading
- `src/modbus.rs` — Modbus protocol layer
- `src/poller.rs` — tag polling scheduler
- `src/scheduler/` — time-based task scheduler, alarm processing, merge logic
- `src/script.rs` / `src/script_cache.rs` — Lua script execution
- `src/db.rs` / `src/db_queries.rs` — PostgreSQL access
- `src/types.rs` — core data structures
- `src/udp_transport.rs` — UDP transport for RTU over UDP

## Configuration

`ss4.toml` — database connections, modbus objects, polling intervals, script bindings.
