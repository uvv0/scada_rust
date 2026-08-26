# im1

SCADA simulator implementing IEC 60870-5-104 / Modbus protocols for industrial automation testing.

## Features

- **IEC 60870-5-104** — standard telecontrol protocol for power systems and SCADA
- **Modbus TCP** — slave/server mode
- **Async runtime** — tokio-based concurrent connection handling
- **Configuration** — TOML-based point database and connection settings

## Use Cases

- RTU simulator for testing SCADA master stations
- Protocol gateway / converter
- Data acquisition simulator for power substations

## Architecture

- `src/main.rs` — entry point and config loading
- `src/imm.rs` — IEC 60870-5-104 protocol implementation (ASDU handling, link layer)
- `src/server_async.rs` — async TCP server with connection management

## Configuration

`im1.toml` — server port, information object addresses, ASDU types, monitoring interval.

## Testing

- `test_elam.py` — Python test client for protocol verification
- `test_elam.ps1` — PowerShell test script
