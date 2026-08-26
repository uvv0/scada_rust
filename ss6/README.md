# ss6 (Rust)

Web viewer for KPZ windows, live values and write commands.

## Purpose
`ss6` provides a browser UI for:
- selecting `KPZ -> Group -> Reg`
- viewing charts and window previews
- reading values from DB or directly from device via Modbus/UDP
- sending TU (`FC5`) and write commands (`FC6/FC16`)

## Auth
- Web users are stored in `public.web_users`
- `ss6` now supports both password formats:
  - legacy `SHA-256(salt:password)`
  - current `argon2id` PHC hashes
- New bootstrap users created by `ss6` are stored in `argon2id`
- This keeps compatibility with accounts managed in `ss7`

## Compatibility With ss7
- UI window lists are built from real windows in `ui.kpz_window` first, then from template-based bindings
- For real windows (`id > 0`) bindings are loaded from `ui.kpz_window_reg_binding` and `ui.kpz_window_text_item`
- `ss7` now stores web-account passwords in `argon2id`; `ss6` accepts those accounts without schema changes

## Preview
- Register widgets show analog colors by alarm-rule and discrete `0/1` state
- Text items without `reg_id` are rendered as static labels and do not participate in polling

## Real Polling
- Status shows `ip/modem`
- Request/response trace shows `TX` then `RX` or `ERR`
- Commands are glued into shared UDP packets when possible
- Waiting timeout: `5000 ms`

## Run
```powershell
cargo run
```

or

```powershell
cargo build --release
.\target\release\ss6.exe
```

Service address: `http://127.0.0.1:8097`
