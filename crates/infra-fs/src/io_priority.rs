//! Interactive reads (pictures, 3D files) go first: a background scan pauses while one is waiting or in flight, so a
//! NAS connection busy with a scan still answers the user quickly.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Longest pause per folder, so a scan always keeps moving even while pictures load without end.
const MAX_PAUSE: Duration = Duration::from_millis(250);
const POLL: Duration = Duration::from_millis(10);

#[derive(Debug, Default)]
pub struct IoPriority {
    foreground: AtomicUsize,
}

/// Held while an interactive read waits or runs.
pub struct Foreground(Arc<IoPriority>);

impl Drop for Foreground {
    fn drop(&mut self) {
        self.0.foreground.fetch_sub(1, Ordering::Relaxed);
    }
}

impl IoPriority {
    /// Marks an interactive read until the returned guard is dropped.
    #[must_use]
    pub fn foreground(self: &Arc<Self>) -> Foreground {
        self.foreground.fetch_add(1, Ordering::Relaxed);
        Foreground(Arc::clone(self))
    }

    /// Called by background work before each read: waits while interactive reads are pending, up to [`MAX_PAUSE`].
    pub fn yield_to_foreground(&self) {
        let start = Instant::now();
        while self.foreground.load(Ordering::Relaxed) > 0 && start.elapsed() < MAX_PAUSE {
            thread::sleep(POLL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_work_waits_for_interactive_reads_but_never_long() {
        let priority = Arc::new(IoPriority::default());
        let start = Instant::now();
        priority.yield_to_foreground();
        assert!(start.elapsed() < Duration::from_millis(5), "nothing in the foreground: no wait");

        let guard = priority.foreground();
        let start = Instant::now();
        priority.yield_to_foreground();
        assert!(start.elapsed() >= MAX_PAUSE, "waits while a picture loads");
        assert!(start.elapsed() < MAX_PAUSE * 4, "but only a short while");
        drop(guard);
        let start = Instant::now();
        priority.yield_to_foreground();
        assert!(start.elapsed() < Duration::from_millis(5), "the guard released it");
    }
}
