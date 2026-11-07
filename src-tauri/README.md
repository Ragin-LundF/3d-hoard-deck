# hoarddeck-desktop (src-tauri)

Tauri 2 shell: maps IPC calls to use cases (`commands.rs`), use-case results to DTOs (`dto.rs`), pushes
scan progress as typed events (`events.rs`) and serves library files read-only through `hoard://` (`protocol.rs`).
No logic beyond mapping and wiring. TypeScript bindings are generated into `ui/src/ipc/bindings.ts`:

    UPDATE_BINDINGS=1 cargo test -p hoarddeck-desktop export_bindings   # plain `cargo test` fails while they are stale
