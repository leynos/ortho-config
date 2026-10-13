//! Stage and duration tracking for one artefact publication.
//!
//! `publish` owns the sequence — lock, fragment, merge, render, prune,
//! publish — and this module owns the question of *how far it got*. Keeping
//! the two apart is what lets every step route both its success and its
//! failure through one place, so the stage a failing build reports is set by
//! the step that actually failed rather than by the caller's position in a `?`
//! chain.
//!
//! An [`Observation`] is inert when the build did not opt in: it records
//! nothing and writes nothing. The steps do not branch on that, because a
//! branch per step is a branch that can drift; the gating lives at the single
//! point where the report is written.

use std::cell::Cell;
use std::path::Path;
use std::time::Instant;

use super::artefact_report::{
    ErrorCategory, Outcome, PublicationReport, ReportConfig, Stage, epoch_millis, write,
};
use super::artefact_wait_marker::clear_wait;

/// Identifies the publication an [`Observation`] describes.
///
/// These four travel together on every report — where it is written, and which
/// crate, deriving type, and source file it came from — so they are grouped
/// rather than threaded one by one through `begin` and every step that might
/// need to name the expansion.
pub(super) struct Subject<'a> {
    /// Artefact root the report is written below.
    pub(super) root: &'a Path,
    /// Crate whose expansion published.
    pub(super) crate_name: &'a str,
    /// Deriving type, naming this expansion among the crate's several.
    pub(super) ident: &'a str,
    /// Recorded source file of this expansion.
    pub(super) source_file: &'a str,
}

/// Records how far one publication got and how long it took.
///
/// Every step routes both its success and its failure through here, so the
/// stage a failing build reports is set by the step that actually failed rather
/// than by the caller's position in a `?` chain.
pub(super) struct Observation<'a> {
    /// Which publication this observation describes.
    subject: Subject<'a>,
    /// Observation inputs the entry boundary resolved.
    config: ReportConfig,
    /// When this publication first attempted the lock.
    started: Instant,
    /// Stage most recently entered, shared so a step's closure can set it
    /// without holding a mutable borrow of the whole observation.
    stage: Cell<Stage>,
    /// When the lock was granted, once it has been.
    granted: Cell<Option<Instant>>,
    /// Bounded category of the failure, once one has been recorded.
    failure: Cell<Option<ErrorCategory>>,
    /// Whether the terminal report has already been written.
    ///
    /// Several nested steps can observe the same escaping failure — the outer
    /// `write_all` finally observes whatever an inner step already observed —
    /// and a report is a single file named for one publication. Writing it once
    /// keeps the file describing one terminal state instead of being rewritten
    /// by each layer the error unwinds through.
    finished: Cell<bool>,
}

impl<'a> Observation<'a> {
    /// Starts observing one publication before it attempts the lock.
    pub(super) fn begin(subject: Subject<'a>, config: ReportConfig) -> Self {
        Self {
            subject,
            config,
            started: Instant::now(),
            stage: Cell::new(Stage::LockWait),
            granted: Cell::new(None),
            failure: Cell::new(None),
            finished: Cell::new(false),
        }
    }

    /// Reports whether this build writes observations at all.
    pub(super) const fn is_on(&self) -> bool {
        self.config.reporting.is_on()
    }

    /// Returns the artefact root this publication writes below.
    pub(super) const fn root(&self) -> &'a Path {
        self.subject.root
    }

    /// Returns the wait after which a blocked acquisition announces itself.
    pub(super) const fn threshold(&self) -> std::time::Duration {
        self.config.threshold
    }

    /// Returns how often a still-blocked waiter refreshes its announcement.
    pub(super) const fn refresh(&self) -> std::time::Duration {
        self.config.refresh
    }

    /// Records the stage a step is about to run.
    pub(super) fn enter(&self, stage: Stage) {
        self.stage.set(stage);
    }

    /// Records when the lock was granted and retires the wait marker.
    ///
    /// Retiring here covers every path on which the lock is *acquired*. A
    /// failed acquisition never reaches this, so `publish` retires the marker
    /// on that path instead: a marker outliving the wait it describes would
    /// make a healthy later build look blocked.
    pub(super) fn granted(&self, at: Instant) {
        self.granted.set(Some(at));
        if self.is_on() {
            clear_wait(self.subject.root);
        }
    }

    /// Retires this process's wait marker without recording a grant.
    ///
    /// The acquisition failed, so no lock was ever held; the marker still
    /// described a wait that has now ended.
    pub(super) fn abandon_wait(&self) {
        if self.is_on() {
            clear_wait(self.subject.root);
        }
    }

    /// Records the bounded category of a failure before it escapes.
    ///
    /// The first classification wins. A step that fails deep inside the
    /// publication knows what it was doing; each layer the error then unwinds
    /// through would classify it again, and only the innermost layer can tell a
    /// serialization failure from the I/O failure its caller would guess at.
    pub(super) fn failed(&self, category: ErrorCategory) {
        if self.failure.get().is_none() {
            self.failure.set(Some(category));
        }
    }

    /// Writes the finished observation for the stage the run reached.
    ///
    /// Called on both the success and failure paths, so a build that never
    /// reached the merge records the stage it died at rather than a generic
    /// failure — the difference between a report that names the fault and one
    /// that only confirms something went wrong.
    pub(super) fn finish(&self, outcome: Outcome) {
        if !self.is_on() || self.finished.replace(true) {
            return;
        }
        let lock_wait = self
            .granted
            .get()
            .map_or_else(|| self.started.elapsed(), |at| at - self.started);
        write(
            self.subject.root,
            &PublicationReport {
                pid: std::process::id(),
                crate_name: self.subject.crate_name,
                ident: self.subject.ident,
                source_file: self.subject.source_file,
                stage: self.stage.get(),
                outcome,
                error: self.failure.get(),
                lock_wait,
                elapsed: self.started.elapsed(),
                written_at_ms: epoch_millis(),
            },
        );
    }
}

/// Runs one observed publication step, recording a failure before it escapes.
///
/// Routing every step through here is what makes the reported stage the one
/// that actually failed: a `?` chain alone would leave `publish` recording
/// whatever stage the caller last set. The category travels with the failure so
/// the report carries a bounded classification of what went wrong, not only
/// where.
pub(super) fn observe<T>(
    observation: &Observation<'_>,
    category: ErrorCategory,
    step: impl FnOnce() -> syn::Result<T>,
) -> syn::Result<T> {
    match step() {
        Ok(value) => Ok(value),
        Err(error) => {
            observation.failed(category);
            observation.finish(Outcome::Failed);
            Err(error)
        }
    }
}
