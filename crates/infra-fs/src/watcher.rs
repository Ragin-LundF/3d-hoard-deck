//! Change events for library folders (`FSEvents`, inotify, `ReadDirectoryChangesW`). Reads only.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use hoarddeck_application::ports::{ChangeWatcher, SettledChange};
use hoarddeck_domain::RelPath;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// More changed folders than this are scanned as a whole instead.
const MAX_DIRS: usize = 2000;

#[derive(Default)]
struct Changed {
    last: Option<Instant>,
    dirs: BTreeSet<RelPath>,
    everything: bool,
}

/// The root as the library names it, and its real path that events use.
type Roots = Arc<Mutex<Vec<(PathBuf, PathBuf)>>>;
type ChangedRoots = Arc<Mutex<HashMap<PathBuf, Changed>>>;

pub struct NotifyWatcher {
    watcher: Mutex<Option<RecommendedWatcher>>,
    roots: Roots,
    changed: ChangedRoots,
}

impl NotifyWatcher {
    #[must_use]
    pub fn new() -> Self {
        let roots: Roots = Arc::default();
        let changed: ChangedRoots = Arc::default();
        let (event_roots, event_changed) = (roots.clone(), changed.clone());
        // Without a system watcher every library falls back to timed rescans.
        let watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            let roots = event_roots.lock().unwrap_or_else(PoisonError::into_inner);
            let mut changed = event_changed.lock().unwrap_or_else(PoisonError::into_inner);
            record(&roots, &mut changed, event);
        })
        .ok();
        Self { watcher: Mutex::new(watcher), roots, changed }
    }
}

impl Default for NotifyWatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Notes which folders below which root changed. Lost events (overflow) or events without paths mark every root.
fn record(roots: &[(PathBuf, PathBuf)], changed: &mut HashMap<PathBuf, Changed>, event: notify::Result<Event>) {
    let now = Instant::now();
    let event = match event {
        Ok(event) if matches!(event.kind, EventKind::Access(_)) => return,
        Ok(event) if !event.need_rescan() && !event.paths.is_empty() => event,
        _ => {
            for (root, _) in roots {
                let entry = changed.entry(root.clone()).or_default();
                (entry.last, entry.everything) = (Some(now), true);
            }
            return;
        }
    };
    for path in event.paths.iter().filter(|path| !is_own_file(path)) {
        let Some((root, real)) = roots.iter().find(|(_, real)| path.starts_with(real)) else { continue };
        let entry = changed.entry(root.clone()).or_default();
        entry.last = Some(now);
        // The folder holding the changed entry is listed again; a file changed in place leaves its mtime alone.
        match path.parent().and_then(|dir| relative(real, dir)) {
            Some(dir) if entry.dirs.len() < MAX_DIRS => {
                entry.dirs.insert(dir);
            }
            _ => entry.everything = true,
        }
    }
}

fn relative(root: &Path, dir: &Path) -> Option<RelPath> {
    let rest = dir.strip_prefix(root).ok()?;
    let segments: Vec<String> = rest.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
    RelPath::new(segments.join("/")).ok()
}

impl ChangeWatcher for NotifyWatcher {
    fn watch(&self, root: &str) -> Result<(), String> {
        let mut watcher = self.watcher.lock().unwrap_or_else(PoisonError::into_inner);
        let watcher = watcher.as_mut().ok_or("no file system watcher on this system")?;
        // Events name real paths; a library added through a symlink must still match them.
        let real = std::fs::canonicalize(root).map_err(|error| format!("{root}: {error}"))?;
        if let Err(error) = watcher.watch(&real, RecursiveMode::Recursive) {
            // inotify may have watched part of the tree before hitting its limit; that part is given back.
            let _ = watcher.unwatch(Path::new(root));
            return Err(format!("{root}: {error}"));
        }
        self.roots.lock().unwrap_or_else(PoisonError::into_inner).push((PathBuf::from(root), real));
        Ok(())
    }

    fn unwatch(&self, root: &str) {
        let mut roots = self.roots.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(watcher) = self.watcher.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
            for (_, real) in roots.iter().filter(|(given, _)| given == Path::new(root)) {
                let _ = watcher.unwatch(real);
            }
        }
        roots.retain(|(given, _)| given != Path::new(root));
    }

    fn settled(&self, quiet: Duration) -> Vec<SettledChange> {
        let mut changed = self.changed.lock().unwrap_or_else(PoisonError::into_inner);
        let roots: Vec<PathBuf> = changed
            .iter()
            .filter(|(_, change)| change.last.is_some_and(|last| last.elapsed() >= quiet))
            .map(|(root, _)| root.clone())
            .collect();
        roots
            .into_iter()
            .filter_map(|root| {
                let change = changed.remove(&root)?;
                Some(SettledChange {
                    root: root.to_string_lossy().into_owned(),
                    dirs: change.dirs.into_iter().collect(),
                    everything: change.everything,
                })
            })
            .collect()
    }
}

/// The app's own metadata files would otherwise trigger a rescan after every save.
fn is_own_file(path: &Path) -> bool {
    let name = path.file_name().map(|name| name.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    // A folder's metadata file is named only `.3dhoarddeck`, which `extension()` does not see.
    name.ends_with(".3dhoarddeck")
        || name == ".ds_store"
        || path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("tmp"))
}

#[cfg(test)]
mod tests {
    use notify::event::{CreateKind, Flag};

    use super::*;

    #[test]
    fn events_name_their_folder_and_an_overflow_marks_everything() {
        let roots = vec![(PathBuf::from("/lib"), PathBuf::from("/real/lib"))];
        let mut changed = HashMap::new();
        let created = Event::new(EventKind::Create(CreateKind::File)).add_path(PathBuf::from("/real/lib/Minis/a.stl"));
        record(&roots, &mut changed, Ok(created));
        let own = Event::new(EventKind::Create(CreateKind::File)).add_path(PathBuf::from("/real/lib/x/.3dhoarddeck"));
        record(&roots, &mut changed, Ok(own));
        let dirs: Vec<String> = changed[Path::new("/lib")].dirs.iter().map(ToString::to_string).collect();
        assert_eq!(dirs, ["Minis"]);
        assert!(!changed[Path::new("/lib")].everything);
        record(&roots, &mut changed, Ok(Event::new(EventKind::Other).set_flag(Flag::Rescan)));
        assert!(changed[Path::new("/lib")].everything);
    }

    #[test]
    fn a_new_file_is_reported_once_after_it_settled() {
        let dir = tempfile::tempdir().expect("temp dir");
        // On macOS the temp dir sits behind the `/var` symlink, like a library added through a symlink.
        let root = dir.path().to_str().expect("utf-8");
        let watcher = NotifyWatcher::new();
        watcher.watch(root).expect("watch");
        std::thread::sleep(Duration::from_millis(300));
        std::fs::create_dir(Path::new(root).join("Minis")).expect("dir");
        std::fs::write(Path::new(root).join("Minis/dragon.stl"), b"solid").expect("write");
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut settled = Vec::new();
        while settled.is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
            settled = watcher.settled(Duration::from_millis(200));
        }
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].root, root);
        // macOS reports the file in `Minis`; inotify (Linux) watches the new folder only after it appeared, so it may
        // report just the root, which is enough: the root is listed again and finds `Minis` as a new folder.
        let reported = |name: &str| settled[0].dirs.iter().any(|dir| dir.as_str() == name);
        assert!(reported("Minis") || reported("") || settled[0].everything, "{:?}", settled[0]);
        assert_eq!(watcher.settled(Duration::ZERO), Vec::new(), "reported once");
    }
}
