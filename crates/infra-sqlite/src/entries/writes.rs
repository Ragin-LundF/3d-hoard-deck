//! How each walk result changes the stored entries.

use hoarddeck_application::scan::{Generation, ScanItem};
use hoarddeck_domain::{Entry, EntryKind, LibraryId, RelPath};
use rusqlite::{Transaction, params};

use crate::rows::{subtree_bounds, to_i64};

pub(super) fn apply_item(
    transaction: &Transaction<'_>,
    library: LibraryId,
    generation: Generation,
    item: &ScanItem,
) -> rusqlite::Result<()> {
    match item {
        ScanItem::Listed { dir, mtime, children } => {
            transaction
                .prepare_cached(
                    "INSERT INTO entries (library_id, path, parent, is_dir, kind, size, mtime, listed_mtime, generation)
                     VALUES (?1, ?2, ?3, 1, 'dir', 0, ?4, ?4, ?5)
                     ON CONFLICT (library_id, path) DO UPDATE SET
                        is_dir = 1, kind = 'dir', mtime = ?4, listed_mtime = ?4, generation = ?5",
                )?
                .execute(params![library.0, dir.as_str(), parent_of(dir), mtime, generation.0])?;
            for child in children {
                upsert_child(transaction, library, generation, child)?;
            }
        }
        ScanItem::Unchanged { dir } => {
            // Two statements instead of an OR, each on its index; the planner skips entries_parent in UPDATEs unless told.
            transaction
                .prepare_cached("UPDATE entries SET generation = ?3 WHERE library_id = ?1 AND path = ?2")?
                .execute(params![library.0, dir.as_str(), generation.0])?;
            transaction
                .prepare_cached(
                    "UPDATE entries INDEXED BY entries_parent SET generation = ?3 WHERE library_id = ?1 AND parent = ?2 AND is_dir = 0",
                )?
                .execute(params![library.0, dir.as_str(), generation.0])?;
        }
        ScanItem::Failed { dir, .. } => keep_subtree(transaction, library, generation, dir)?,
    }
    Ok(())
}

/// Marks everything stored below an unreadable folder as seen, so the end of the scan does not delete it.
fn keep_subtree(
    transaction: &Transaction<'_>,
    library: LibraryId,
    generation: Generation,
    dir: &RelPath,
) -> rusqlite::Result<()> {
    if dir.is_root() {
        transaction
            .execute("UPDATE entries SET generation = ?2 WHERE library_id = ?1", params![library.0, generation.0])?;
        return Ok(());
    }
    let (low, high) = subtree_bounds(dir);
    transaction
        .prepare_cached("UPDATE entries SET generation = ?3 WHERE library_id = ?1 AND path = ?2")?
        .execute(params![library.0, dir.as_str(), generation.0])?;
    transaction
        .prepare_cached("UPDATE entries SET generation = ?4 WHERE library_id = ?1 AND path >= ?2 AND path < ?3")?
        .execute(params![library.0, low, high, generation.0])?;
    Ok(())
}

/// Files are fully replaced. A subfolder only gets marked seen; its `listed_mtime` changes only when it is listed itself.
fn upsert_child(
    transaction: &Transaction<'_>,
    library: LibraryId,
    generation: Generation,
    child: &Entry,
) -> rusqlite::Result<()> {
    let parent = parent_of(&child.path);
    match child.kind {
        EntryKind::Dir => transaction
            .prepare_cached(
                "INSERT INTO entries (library_id, path, parent, is_dir, kind, size, mtime, listed_mtime, generation)
                 VALUES (?1, ?2, ?3, 1, 'dir', 0, ?4, NULL, ?5)
                 ON CONFLICT (library_id, path) DO UPDATE SET
                    listed_mtime = CASE WHEN is_dir = 1 THEN listed_mtime END, is_dir = 1, kind = 'dir', generation = ?5",
            )?
            .execute(params![library.0, child.path.as_str(), parent, child.mtime, generation.0])?,
        EntryKind::File(kind) => transaction
            .prepare_cached(
                "INSERT INTO entries (library_id, path, parent, is_dir, kind, size, mtime, listed_mtime, generation)
                 VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, NULL, ?7)
                 ON CONFLICT (library_id, path) DO UPDATE SET
                    is_dir = 0, kind = ?4, size = ?5, mtime = ?6, listed_mtime = NULL, generation = ?7",
            )?
            .execute(params![library.0, child.path.as_str(), parent, kind.code(), to_i64(child.size), child.mtime, generation.0])?,
    };
    Ok(())
}

fn parent_of(path: &RelPath) -> String {
    path.parent().map(String::from).unwrap_or_default()
}
