# Project Instructions

## Build And Verify

- After every code change, always run `cargo check`.
- After `cargo check`, always build the release binary with `cargo build --release`.
- The release binary path is always `target/release/ss7.exe`.
- Do not switch to an alternate target directory unless the user explicitly asks for it.
- If `target/release/ss7.exe` is locked and cannot be replaced, report that fact and wait for user direction.

## Communication

- Treat rebuilding `target/release/ss7.exe` as part of the normal workflow after changes.
- Do not ask again whether `cargo check` should be run as part of the normal edit cycle.
- Do not ask again whether `cargo build --release` should be run as part of the normal edit cycle.
