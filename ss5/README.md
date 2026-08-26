# ss5

Embedded Rust SCADA firmware for STM32H7 microcontrollers.

## Overview

Firmware for industrial RTU/PLC devices running on ARM Cortex-M7. Provides SCADA data acquisition, Lua scripting, Modbus, and web interface on bare metal (no OS).

## Features

- **Modbus TCP/RTU** — slave/server implementation
- **Lua VM** — embedded Lua 5.4.8 for user scripts running in XIP mode
- **Web server** — lightweight HTTP server for HMI and configuration
- **Tag polling** — scheduled data acquisition from connected devices
- **Archive storage** — ring archives in external SPI flash (W25Q128)
- **Thread profiler** — runtime execution time monitoring

## Hardware

- MCU: STM32H7 series
- External flash: W25Q128 (SPI4)
- Ethernet: RMII with LAN8742 PHY
- UART: multiple ports for RS-485/RS-232

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
- `src/script.rs` — Lua script integration
- `src/ui/` — embedded UI rendering
- `src/app/windows/` — UI window implementations (alarm, archive, graph, editor)

## Firmware modules

- `board.c` / `drv_*.c` — HAL drivers (GPIO, ETH, USART)
- `web_server.c` — HTTP server and Lua web editor
- `lua_vm_module.c` — Lua VM integration layer
- `tag_poll_scheduler.c` — polling scheduler
- `thread_profiler.c` — execution profiler
- `qspi_modules.c` — external module management
- `module_service_api.c` — module service API
