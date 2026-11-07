# hoarddeck-infra-tantivy

Search adapter: a Tantivy index in `<index folder>/search/`, derived from SQLite and always rebuildable.
Fuzzy prefix matching on title and path words, exact title matches ranked first, folder scope as a facet filter.
Allowed dependencies: hoarddeck-domain, hoarddeck-application, tantivy.
