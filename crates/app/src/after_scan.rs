use std::sync::{Arc, OnceLock, Weak};

use hoarddeck_application::ai::SuggestTags;
use hoarddeck_application::ports::EventSink;
use hoarddeck_application::scan::{ScanPhase, ScanProgress};
use hoarddeck_domain::LibraryId;

/// Passes every event on, and when a whole-library scan ends, lets the AI ask about new models in its folders (if the
/// user switched that on). The scan does not know about AI; this is the one place both meet.
pub(crate) struct AfterScan {
    pub(crate) inner: Arc<dyn EventSink>,
    pub(crate) suggest: OnceLock<Weak<SuggestTags>>,
}

impl EventSink for AfterScan {
    fn scan_progress(&self, progress: &ScanProgress) {
        self.inner.scan_progress(progress);
        // `partial` is a folder scan or a regroup after a rule edit; only real library scans bring new models.
        if progress.phase == ScanPhase::Done
            && !progress.partial
            && let Some(suggest) = self.suggest.get().and_then(Weak::upgrade)
        {
            suggest.after_scan(progress.library);
        }
    }

    fn duplicates_progress(&self, done: usize, total: usize) {
        self.inner.duplicates_progress(done, total);
    }

    fn suggest_progress(&self, library: LibraryId, done: usize, total: usize, failed: usize, running: bool) {
        self.inner.suggest_progress(library, done, total, failed, running);
    }
}
