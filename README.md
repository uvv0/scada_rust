# scada_rust

Collection of SCADA (Supervisory Control and Data Acquisition) projects written in Rust.

## Projects

| Directory | Description |
|-----------|-------------|
| [ss4](ss4/) | SCADA server — Modbus poller, scheduler, Lua scripts, PostgreSQL, MQTT |
| [ss5](ss5/) | Embedded SCADA firmware — STM32H7, Modbus, Lua VM, web server |
| [ss6](ss6/) | SCADA web server — Modbus master, REST API, WebSocket, SQLite, web UI |
| [ss7](ss7/) | SCADA desktop app — egui, accounts, IO, templates, scripts, HMI editor |
| [im1](im1/) | Protocol server — IEC 60870-5-104 and Modbus TCP |

## Common Technologies

- **Language**: Rust (with C bindings where needed)
- **Protocols**: Modbus TCP/RTU, IEC 60870-5-104, MQTT
- **Databases**: PostgreSQL, SQLite
- **UI**: egui (desktop), Qt (legacy), embedded web server
- **Embedded**: STM32H7, IAR toolchain, RTIC framework
- **Async**: tokio, async-std
