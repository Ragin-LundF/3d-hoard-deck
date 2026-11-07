# 3D Hoard Deck

Desktop library for 3D-printable files (STL, 3MF, OBJ …) that indexes your folders in place.
Libraries are read-only by default; all app data lives in the index folder (default `~/.3dhoarddeck`).

## Workspace

| Crate | Layer | Responsibility |
|---|---|---|
| `crates/domain` | Domain | Paths, file classification (content sniffing), name parsing, catalog types. No I/O. |
| `crates/application` | Application | Use cases (`ScanLibrary`, `SearchModels`, `BrowseFolder`, …), port traits, model detection rules. |
| `crates/infra-fs` | Adapter | Parallel read-only walker with incremental rescans; index location bootstrap. |
| `crates/infra-sqlite` | Adapter | SQLite index of record (entries, folders, models). |
| `crates/infra-tantivy` | Adapter | Tantivy search index, rebuilt from SQLite when needed. |
| `crates/app` | Composition root | Builds adapters once and injects them into the use cases. |
| `crates/cli` | Tool | Phase 0 spike tool `hoarddeck` (see its README). |
| `src-tauri` | Shell | Tauri commands, events and the `hoard://` file protocol. |
| `ui` | Frontend | React + TypeScript; renders only, talks to the shell through generated bindings. |

Dependencies point inward only; the compiler enforces it through the crate graph.

## Develop

    npm install && npm --prefix ui install       # once
    npm run dev                                  # desktop app with hot reload
    cargo test --workspace
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all
    npm --prefix ui run build && npm --prefix ui test
    UPDATE_BINDINGS=1 cargo test -p hoarddeck-desktop export_bindings   # regenerate ui/src/ipc/bindings.ts after DTO changes

`HOARDDECK_INDEX_DIR=/some/dir` points the app or CLI at another index folder (demos, tests).

## Status

- Milestone 1: Rust core + spike CLI.
- Milestone 2: Tauri shell and UI: folder browsing with breadcrumbs and lazy tree, "everything inside", fuzzy search with
  folder scope, virtualized grid with folder covers, model page with images and files, dark/light theme, mark a folder
  as model or collection (right-click), add library and rescan.
- Next: cached thumbnails (images are served at full size for now), 3D viewer, open in slicer, collection presets.
