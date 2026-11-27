//! Imports that run in the background, so a large one can report progress.
//!
//! A job lives only in memory: it is a view of work in this process, not a
//! record. A restart forgets it, and the set it was making is either there
//! (it finished) or not (it did not).

use crate::error::AppError;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// How long one import may run before it is given up on.
pub const MAX_RUN: Duration = Duration::from_secs(10 * 60);
/// A finished job is kept this long, so a page that comes back can still read it.
const KEEP_DONE: Duration = Duration::from_secs(60 * 60);
/// At most this many jobs are kept, oldest finished first to go.
const MAX_JOBS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Running,
    Done,
    Failed,
}

/// What a job is doing, as the page polls it.
#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub state: State,
    pub set_code: String,
    pub set_name: String,
    pub cards_total: usize,
    pub cards_done: usize,
    pub pictures_total: usize,
    /// Pictures fetched and kept.
    pub pictures_done: usize,
    /// Pictures tried, fetched or not: what the bar shows.
    pub pictures_tried: usize,
    /// Pictures that could not be fetched, named so they can be retried.
    pub failures: Vec<String>,
    /// The set, once it exists.
    pub set_id: Option<Uuid>,
    /// Why it failed, if it did.
    pub error: Option<String>,
}

struct Job {
    owner: Uuid,
    progress: Progress,
    started: Instant,
    finished: Option<Instant>,
}

#[derive(Clone, Default)]
pub struct Imports {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    jobs: HashMap<Uuid, Job>,
    /// The running job of each account: one import at a time.
    running: HashMap<Uuid, Uuid>,
}

impl Imports {
    /// Starts a job for `owner`, refusing a second while they have one running.
    pub fn start(&self, owner: Uuid, progress: Progress) -> Result<Uuid, AppError> {
        let mut inner = self.inner.lock().expect("the imports lock is not poisoned");
        inner.forget_old();
        if inner.running.contains_key(&owner) {
            return Err(AppError::conflict("An import is already running."));
        }
        let id = Uuid::new_v4();
        inner.jobs.insert(
            id,
            Job {
                owner,
                progress,
                started: Instant::now(),
                finished: None,
            },
        );
        inner.running.insert(owner, id);
        Ok(id)
    }

    /// Changes a job's progress, if it is still there.
    pub fn update(&self, id: Uuid, change: impl FnOnce(&mut Progress)) {
        let mut inner = self.inner.lock().expect("the imports lock is not poisoned");
        if let Some(job) = inner.jobs.get_mut(&id) {
            change(&mut job.progress);
        }
    }

    /// Marks a job finished, with the set it made or why it could not.
    pub fn finish(&self, id: Uuid, owner: Uuid, outcome: Result<Uuid, String>) {
        let mut inner = self.inner.lock().expect("the imports lock is not poisoned");
        if let Some(job) = inner.jobs.get_mut(&id) {
            match outcome {
                Ok(set) => {
                    job.progress.set_id = Some(set);
                    job.progress.state = State::Done;
                }
                Err(error) => {
                    job.progress.error = Some(error);
                    job.progress.state = State::Failed;
                }
            }
            job.finished = Some(Instant::now());
        }
        // Only this job's own entry: a newer one (after this was abandoned) stays.
        if inner.running.get(&owner) == Some(&id) {
            inner.running.remove(&owner);
        }
    }

    /// One job, if it is the account's. Someone else's answers as if it were not
    /// there, which is how every other lookup treats another account's things.
    pub fn get(&self, owner: Uuid, id: Uuid) -> Option<Progress> {
        let inner = self.inner.lock().expect("the imports lock is not poisoned");
        inner
            .jobs
            .get(&id)
            .filter(|job| job.owner == owner)
            .map(|job| job.progress.clone())
    }
}

impl Inner {
    /// Drops jobs nobody will ask about again: finished long ago, or beyond the
    /// last hundred. A running job older than the cap was abandoned (its task
    /// died without finishing), so it is dropped too, freeing the account.
    fn forget_old(&mut self) {
        let now = Instant::now();
        let abandoned = MAX_RUN + Duration::from_secs(60);
        self.jobs.retain(|_, job| match job.finished {
            Some(at) => now.duration_since(at) < KEEP_DONE,
            None => now.duration_since(job.started) < abandoned,
        });
        self.running
            .retain(|_, id| self.jobs.get(id).is_some_and(|job| job.finished.is_none()));

        if self.jobs.len() > MAX_JOBS {
            let mut finished: Vec<(Uuid, Instant)> = self
                .jobs
                .iter()
                .filter_map(|(id, job)| job.finished.map(|at| (*id, at)))
                .collect();
            finished.sort_by_key(|(_, at)| *at);
            for (id, _) in finished.into_iter().take(self.jobs.len() - MAX_JOBS) {
                self.jobs.remove(&id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress() -> Progress {
        Progress {
            state: State::Running,
            set_code: "IMP".into(),
            set_name: "Imported".into(),
            cards_total: 3,
            cards_done: 0,
            pictures_total: 2,
            pictures_done: 0,
            pictures_tried: 0,
            failures: Vec::new(),
            set_id: None,
            error: None,
        }
    }

    #[test]
    fn one_import_at_a_time_per_account() {
        let imports = Imports::default();
        let me = Uuid::new_v4();
        let id = imports.start(me, progress()).unwrap();

        let second = imports.start(me, progress()).unwrap_err();
        assert!(second.detail().contains("already"));
        // Another account is not blocked by mine.
        assert!(imports.start(Uuid::new_v4(), progress()).is_ok());

        // Finishing frees the account.
        imports.finish(id, me, Ok(Uuid::new_v4()));
        assert!(imports.start(me, progress()).is_ok());
    }

    #[test]
    fn a_job_is_only_its_owners() {
        let imports = Imports::default();
        let me = Uuid::new_v4();
        let id = imports.start(me, progress()).unwrap();

        assert!(imports.get(me, id).is_some());
        assert!(
            imports.get(Uuid::new_v4(), id).is_none(),
            "another account's job is not there"
        );
        assert!(imports.get(me, Uuid::new_v4()).is_none());
    }

    #[test]
    fn progress_is_reported_and_the_end_is_remembered() {
        let imports = Imports::default();
        let me = Uuid::new_v4();
        let id = imports.start(me, progress()).unwrap();

        imports.update(id, |p| p.pictures_done = 1);
        imports.update(id, |p| p.failures.push("Dog".into()));
        let seen = imports.get(me, id).unwrap();
        assert_eq!(seen.pictures_done, 1);
        assert_eq!(seen.failures, ["Dog"]);
        assert_eq!(seen.state, State::Running);

        let set = Uuid::new_v4();
        imports.finish(id, me, Ok(set));
        let done = imports.get(me, id).unwrap();
        assert_eq!(done.state, State::Done);
        assert_eq!(done.set_id, Some(set));

        // A failure says why.
        let other = imports.start(me, progress()).unwrap();
        imports.finish(other, me, Err("no".into()));
        assert_eq!(imports.get(me, other).unwrap().state, State::Failed);
        assert_eq!(imports.get(me, other).unwrap().error.as_deref(), Some("no"));
    }

    #[test]
    fn updating_a_job_that_is_gone_is_harmless() {
        let imports = Imports::default();
        imports.update(Uuid::new_v4(), |p| p.cards_done = 9);
        imports.finish(Uuid::new_v4(), Uuid::new_v4(), Ok(Uuid::new_v4()));
    }
}
