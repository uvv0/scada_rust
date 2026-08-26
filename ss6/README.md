# ss6

Rust SCADA web server with Modbus integration and REST API.

## Features

- **Modbus TCP** — master/client for polling remote devices
- **REST API** — JSON endpoints for tags, alarms, archives
- **WebSocket** — real-time data streaming to web clients
- **SQLite** — embedded database for configuration and runtime data
- **Web UI** — static HTML/JS frontend served by the application

## Architecture

- `src/main.rs` — entry point, Axum server setup
- `src/web.rs` — HTTP routes and WebSocket handlers
- `src/handlers.rs` — API endpoint implementations
- `src/db.rs` — SQLite data access
- `src/modbus.rs` — Modbus client protocol
- `src/models.rs` — API data structures
- `src/config.rs` — configuration from `ss6.toml`
- `build.rs` — build-time asset embedding

## API

- `GET /api/tags` — list all tags
- `GET /api/tags/{id}` — single tag value
- `POST /api/tags/{id}` — write tag value
- `GET /api/alarms` — alarm list
- `GET /api/archives` — archive data
- `WS /ws` — WebSocket for real-time updates

## Configuration

`ss6.toml` — server port, database path, Modbus connection settings, tag definitions.

## Tests

- `tests/` — JavaScript integration tests (Vitest + Playwright)
