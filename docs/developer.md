# Developing 3D Hoard Deck

3D Hoard Deck is a [Tauri 2](https://tauri.app) desktop app: a Rust core does all scanning, indexing, parsing and
thumbnail work; a React + TypeScript webview only renders.

## Prerequisites

- Rust (stable, 1.90 or newer) via [rustup](https://rustup.rs). On the original Mac it came from Homebrew's
  `rustup` and lives in `/opt/homebrew/opt/rustup/bin`, which is not on the default `PATH`.
- Node.js 22 or newer with npm.
- Platform build tools for Tauri: Xcode command line tools (macOS), WebView2 and the MSVC build tools
  (Windows), `webkit2gtk-4.1` and friends (Linux). See the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```bash
npm install && npm --prefix ui install   # once
npm run dev                              # app with hot reload of the UI
```

`HOARDDECK_INDEX_DIR=/some/folder npm run dev` points the app (and the CLI) at another index folder, handy for
demo data without touching your real index.

### IntelliJ / RustRover

Shared run configurations live in `.run/`: **3D Hoard Deck (dev)**, **Install dependencies**,
**Checks (tests, lint, types)** and **Build release app**. They set `PATH` so cargo is found.

## Checks

Run before every hand-over; CI should run the same on macOS, Windows and Linux.

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
npm --prefix ui run typecheck && npm --prefix ui test && npm --prefix ui run build
```

The TypeScript IPC bindings (`ui/src/ipc/bindings.ts`) are generated from the Rust command signatures.
`cargo test` fails while they are stale; regenerate with:

```bash
UPDATE_BINDINGS=1 cargo test -p hoarddeck-desktop export_bindings
```

## Build an installer

```bash
npm run build
```

Installers end up in `target/release/bundle/` (`dmg`/`app` on macOS, `msi`/NSIS on Windows, `AppImage`/`deb` on
Linux). A debug build without installer: `npm run tauri -- build --debug --no-bundle`.

## Updates

The app asks GitHub for `releases/latest/download/latest.json` a few seconds after start (Settings › Updates can
turn that off). Only **published** releases count, so publishing the draft release is what rolls an update out.

One-time setup, needed before the first release that should be able to update itself:

1. Create the updater key pair (with a password):

   ```bash
   npm run tauri signer generate -- -w ~/.tauri/3d-hoard-deck.key
   ```

2. Put the content of `~/.tauri/3d-hoard-deck.key.pub` into `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`.
3. Add the GitHub secrets `TAURI_SIGNING_PRIVATE_KEY` (content of `~/.tauri/3d-hoard-deck.key`) and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The release workflow then signs the installers and attaches `latest.json`.
4. Back up the private key. Without it, installed apps cannot update themselves any more.

Builds without a key still work: the update hint then only opens the release page. Linux `.deb`/`.rpm` installs
download the new package to Downloads instead of installing it. The release notes (and the "What's new" of the hint)
start with the version's section of `CHANGELOG.md`.

## Architecture

Hexagonal layers, one crate each, so the compiler enforces the dependency direction: the domain depends on
nothing, the application only on the domain, adapters implement the application's ports, and the composition
root wires them together.

| Crate | Layer | Responsibility |
|---|---|---|
| `crates/domain` | Domain | Paths (`RelPath`), file classification and content sniffing, name parsing, catalog types, library settings, slicers. No I/O. |
| `crates/application` | Application | Use cases (one struct, one job) and the port traits they need. Feature modules: `library`, `scan` (detection rules, presets, folder rules), `browse`, `search`, `slicer`. |
| `crates/infra-fs` | Adapter | Parallel read-only walker with mtime-based incremental rescans, index location bootstrap (`~/.3dhoarddeck/config.json`), storage inspection (network / cloud-sync folders). |
| `crates/infra-sqlite` | Adapter | The index of record: `index.sqlite` (WAL), migrations in `schema.rs`, keyset paging, settings. |
| `crates/infra-tantivy` | Adapter | Search: fuzzy prefix matching, NFC normalization, folder facets. Derived from SQLite and rebuilt when its schema changes. |
| `crates/infra-mesh` | Adapter | 3MF reader: core spec, production-extension object parts, Bambu/Orca plate assignment; binary plate format for the viewer. |
| `crates/infra-thumbs` | Adapter | Cover thumbnails (512 px JPEG, EXIF orientation, transparency flattened) in `<index>/thumbnails`. |
| `crates/infra-platform` | Adapter | Starting slicers without a shell, detecting installed slicers per OS. |
| `crates/app` | Composition root | Builds every adapter once and injects them into the use cases (`App`). |
| `crates/cli` | Tool | `hoarddeck`, a read-only command line for spikes and debugging (`scan`, `search`, `browse`, `plates`). |
| `src-tauri` | Shell | Tauri commands (thin mapping to use cases), DTOs (`dto/`), scan events, the `hoard://` file protocol (`protocol/`). |
| `ui` | Frontend | React 19, TypeScript, Vite, TanStack Query and Virtual, three.js for the viewer. Feature folders under `ui/src/features`. |

### Data flow

1. **Scan.** `ScanLibrary` walks a library through `FsLibraryScanner`; unchanged folders (same mtime) are not
   listed again. Raw entries go to SQLite in batches.
2. **Group.** Detection (`application/scan/detect`) turns entries into models and folders: variant folders
   merge, release dates are parsed, covers are chosen, folder rules and presets apply. Changing a rule regroups
   from stored entries without touching the disk.
3. **Index.** Models are written to SQLite (ids stay stable) and pushed to Tantivy.
4. **Show.** The UI calls commands (`browse_folder`, `search_models`, `get_model` …) and loads every picture and
   mesh through `hoard://`: `thumb/…` for cached covers, `<library>/<path>` for files, `plates/…` for 3MF plates.
   A small pool of reader threads serves the viewer first and covers newest-first.

### The rules that matter

- **Library folders are read-only unless the user allows writes**, per library. Tests and tools must never write
  into a real library; use generated fixtures.
- **Every schema change is a new migration** in `crates/infra-sqlite/src/schema.rs`; existing indexes upgrade in
  place.
- **Ports belong to the consumer** (application); adapters never call each other.
- **Commands are thin**: deserialize, call one use case, map to a DTO.
- **Small files**: split a module when it grows past roughly 150 lines.
- Workspace lints: `clippy::pedantic`, `unsafe_code` forbidden, `unwrap_used` denied outside tests.

## Testing

- Unit tests sit next to the code; use cases are tested with in-memory fakes of their ports
  (`crates/application/tests`).
- Adapters have integration tests against temp folders (`crates/*/tests`).
- `crates/app/tests/end_to_end.rs` scans a generated library with the real adapters and checks that the library
  folder is byte-for-byte unchanged afterwards.
- Frontend logic has Vitest tests (`*.test.ts`).

## Debugging helpers

```bash
cargo run --release -p hoarddeck-cli -- --index /tmp/hd add Demo "/path/to/models"
cargo run --release -p hoarddeck-cli -- --index /tmp/hd scan 1 [--full]
cargo run --release -p hoarddeck-cli -- --index /tmp/hd search 1 wolverin type:stl
cargo run --release -p hoarddeck-cli -- plates "/path/to/file.3mf"
```

The CLI only reads libraries; it writes into the index folder given with `--index`.
