//! A few reader threads with two lanes. Viewer requests always go first; covers are served newest first, so when
//! the grid scrolls fast the covers now on screen win over ones that already scrolled away.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex, Once, OnceLock, PoisonError};

/// Parallel file reads; the NAS should not get a whole grid page of requests at once.
const READERS: usize = 4;
/// Older requests beyond this are dropped; their images were unmounted long ago and ask again when shown.
const MAX_WAITING: usize = 512;

/// Called with `true` when the job was dropped from a full queue instead of being run.
pub(super) type Job = Box<dyn FnOnce(bool) + Send>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Lane {
    /// The 3D viewer: one request the user is waiting for.
    Viewer,
    /// Grid covers, gallery thumbnails and images.
    Covers,
}

#[derive(Default)]
struct Jobs {
    viewer: VecDeque<Job>,
    covers: VecDeque<Job>,
}

struct Queue {
    jobs: Mutex<Jobs>,
    ready: Condvar,
}

pub(super) fn run(lane: Lane, job: Job) {
    static QUEUE: OnceLock<Queue> = OnceLock::new();
    static WORKERS: Once = Once::new();
    let queue: &'static Queue =
        QUEUE.get_or_init(|| Queue { jobs: Mutex::new(Jobs::default()), ready: Condvar::new() });
    WORKERS.call_once(|| {
        for _ in 0..READERS {
            std::thread::spawn(move || work(queue));
        }
    });
    let mut jobs = queue.jobs.lock().unwrap_or_else(PoisonError::into_inner);
    match lane {
        Lane::Viewer => jobs.viewer.push_back(job),
        Lane::Covers => {
            jobs.covers.push_back(job);
            if jobs.covers.len() > MAX_WAITING
                && let Some(oldest) = jobs.covers.pop_front()
            {
                oldest(true);
            }
        }
    }
    queue.ready.notify_one();
}

fn work(queue: &Queue) {
    loop {
        let job = {
            let mut jobs = queue.jobs.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
                if let Some(job) = jobs.viewer.pop_front().or_else(|| jobs.covers.pop_back()) {
                    break job;
                }
                jobs = queue.ready.wait(jobs).unwrap_or_else(PoisonError::into_inner);
            }
        };
        job(false);
    }
}
