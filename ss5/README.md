# ss5

Embedded Rust SCADA firmware for microcontrollers.

## Overview

Firmware for industrial RTU/PLC devices. Provides SCADA data acquisition, Modbus, and web interface on bare metal (no OS).

## Features

- **Modbus TCP/RTU** — slave/server implementation
- **Web server** — lightweight HTTP server for HMI and configuration
- **Tag polling** — scheduled data acquisition from connected devices

## Build

- Toolchain: IAR Embedded Workbench for ARM
- Build system: Makefile + custom scripts
- Flash programming: J-Link

## Architecture

- `src/main.rs` — RTIC application entry point
- `src/app.rs` — main application state machine
- `src/modbus.rs` — Modbus protocol stack
- `src/modbus_service.rs` — request handler
- `src/db.rs` — in-memory data storage
- `src/models.rs` — data structures
- `src/ui/` — embedded UI rendering
- `src/app/windows/` — UI window implementations (alarm, archive, graph, editor)
