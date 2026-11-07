# hoarddeck-infra-sqlite

The index of record: one SQLite file (`index.sqlite`, WAL mode) in the index folder. Implements the
`LibraryRepository`, `EntryStore` and `ModelRepository` ports. One writer connection, a small pool of readers.
Allowed dependencies: hoarddeck-domain, hoarddeck-application, rusqlite (bundled), serde_json.
