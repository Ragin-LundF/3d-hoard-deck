use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hoarddeck_domain::{LibraryId, RelPath};

use super::{ScanLibrary, ScanMode};
use crate::AppError;
use crate::ports::{ChangeWatcher, LibraryRepository, StorageInspector, StorageKind};

/// Changes are scanned once the folder stayed quiet this long, so a copy of many files causes one scan.
const QUIET: Duration = Duration::from_secs(3);
/// Network shares send no change events; they are rescanned this often (only changed folders are read).
const SHARE_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// Watched libraries are rescanned this often too, in case the system dropped events unnoticed.
const WATCHED_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// Changes made while the app was closed: one library is scanned this long after the previous one.
const STARTUP_STAGGER: Duration = Duration::from_secs(10);
/// No startup scan for a library scanned this recently; restarting the app should not stir up the NAS each time.
const RECENT_SCAN: Duration = Duration::from_secs(60 * 60);

struct Watched {
    root: String,
    /// Not watched by the system: a network share, or watching failed.
    timed: bool,
    next_scan: Instant,
}

/// What a library's next scan must cover.
#[derive(Default)]
struct Pending {
    dirs: BTreeSet<RelPath>,
    full: bool,
}

#[derive(Default)]
struct State {
    watched: HashMap<LibraryId, Watched>,
    pending: HashMap<LibraryId, Pending>,
    started: bool,
}

/// Keeps libraries current: watched local folders are rescanned after changes, network shares on a timer, and every
/// library once shortly after the start. Driven by [`AutoRescan::tick`] from a background thread.
pub struct AutoRescan {
    libraries: Arc<dyn LibraryRepository>,
    storage: Arc<dyn StorageInspector>,
    watcher: Arc<dyn ChangeWatcher>,
    scan: Arc<ScanLibrary>,
    state: Mutex<State>,
}

impl AutoRescan {
    #[must_use]
    pub fn new(
        libraries: Arc<dyn LibraryRepository>,
        storage: Arc<dyn StorageInspector>,
        watcher: Arc<dyn ChangeWatcher>,
        scan: Arc<ScanLibrary>,
    ) -> Self {
        Self { libraries, storage, watcher, scan, state: Mutex::default() }
    }

    /// Picks up added and removed libraries, then scans those that changed or are due. Returns the scanned ones.
    /// # Errors
    /// Fails when the library list cannot be read. Scan failures (offline, busy) are retried later.
    pub fn tick(&self, now: Instant) -> Result<Vec<LibraryId>, AppError> {
        self.collect(now)?;
        let due: Vec<(LibraryId, Pending)> = {
            let mut state = self.state()?;
            let ids: Vec<LibraryId> = state.pending.keys().copied().collect();
            ids.into_iter().filter_map(|id| state.pending.remove(&id).map(|pending| (id, pending))).collect()
        };
        let mut scanned = Vec::new();
        for (library, pending) in due {
            let mode = if pending.full { ScanMode::Full } else { ScanMode::Incremental };
            let dirs: Vec<RelPath> = pending.dirs.iter().cloned().collect();
            let result = self.scan.execute_with(library, mode, &dirs);
            let mut state = self.state()?;
            if let Err(AppError::LibraryBusy(_)) = result {
                // A running scan may have missed the change, so it stays pending.
                let again = state.pending.entry(library).or_default();
                again.dirs.extend(pending.dirs);
                again.full |= pending.full;
                continue;
            }
            // Other failures (an offline share) wait for the next change or turn instead of retrying every second.
            if let Some(watched) = state.watched.get_mut(&library) {
                watched.next_scan = now + if watched.timed { SHARE_INTERVAL } else { WATCHED_INTERVAL };
            }
            scanned.push(library);
        }
        scanned.sort_by_key(|id| id.0);
        Ok(scanned)
    }

    fn collect(&self, now: Instant) -> Result<(), AppError> {
        let libraries = self.libraries.list()?;
        let mut state = self.state()?;
        let State { watched, pending, started } = &mut *state;
        watched.retain(|id, entry| {
            let keep = libraries.iter().any(|library| library.id == *id && library.root == entry.root);
            if !keep && !entry.timed {
                self.watcher.unwatch(&entry.root);
            }
            keep
        });
        let mut startup = 0_u32;
        for library in &libraries {
            watched.entry(library.id).or_insert_with(|| {
                let local = self.storage.kind(Path::new(&library.root)) == StorageKind::Local;
                let timed = !local || self.watcher.watch(&library.root).is_err();
                // Libraries known at the start get one scan each, staggered, unless scanned recently; ones added later
                // were just scanned.
                let next_scan = if *started || self.scanned_recently(library.id) {
                    now + if timed { SHARE_INTERVAL } else { WATCHED_INTERVAL }
                } else {
                    startup += 1;
                    now + STARTUP_STAGGER * startup
                };
                Watched { root: library.root.clone(), timed, next_scan }
            });
        }
        *started = true;
        for change in self.watcher.settled(QUIET) {
            for (id, _) in watched.iter().filter(|(_, entry)| entry.root == change.root) {
                let next = pending.entry(*id).or_default();
                next.dirs.extend(change.dirs.iter().cloned());
                next.full |= change.everything;
            }
        }
        for (id, entry) in watched.iter() {
            if now >= entry.next_scan {
                pending.entry(*id).or_default();
            }
        }
        Ok(())
    }

    fn scanned_recently(&self, library: LibraryId) -> bool {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
        self.libraries
            .last_scan_at(library)
            .ok()
            .flatten()
            .and_then(|at| u64::try_from(at).ok())
            .is_some_and(|at| now.saturating_sub(at) < RECENT_SCAN.as_secs())
    }

    fn state(&self) -> Result<MutexGuard<'_, State>, AppError> {
        self.state.lock().map_err(|_| AppError::InvalidInput("watch state poisoned".to_owned()))
    }
}
