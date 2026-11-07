# hoarddeck-application

Use cases (one struct, one `execute`) and the port traits they need. Feature modules:
`library`, `scan`, `browse`, `search`. Allowed dependencies: hoarddeck-domain, thiserror, serde, globset.
Never imports an adapter crate and never touches the file system, SQLite or Tantivy directly.
