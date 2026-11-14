//! Test doubles for the artefact-lock boundaries.
//!
//! [`ProcFs`] and [`Clock`] exist so the report's *decisions* can be driven
//! without a real held lock, a real fork, or a real unreadable `/proc` entry.
//! These are the doubles that do the driving: a procfs that answers from a
//! table the test states, and a clock that advances only when the code under
//! test asks it to.
//!
//! They live in one module because more than one suite needs them. A copy per
//! suite would be two fakes that must agree about the boundaries they
//! implement, and the first divergence between them would be invisible.
//!
//! [`ProcFs`]: super::lock_procfs::ProcFs
//! [`Clock`]: super::lock_clock::Clock

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use super::lock_clock::Clock;
use super::lock_evidence::{Descriptor, LockFileIdentity, LockRecord, ProcessId};
use super::lock_procfs::{DescriptorEntries, ProcFs, ProcessStat};

/// A procfs whose every answer the test states up front.
///
/// One field per observation rather than one "read a path" answer, so a test
/// can fail the lock table and leave the descriptor scan answering: the
/// report's branches are only distinguishable if the reads behind them are
/// separable.
///
/// Keys are the rendered values, because that is the surface the evidence
/// newtypes expose. Reaching for the inner integers would mean adding an
/// accessor whose only caller is a test, and the newtypes deliberately do not
/// offer one.
///
/// `Default` is written rather than derived because the lock table is a
/// `Result`, and its empty case is `Ok(no records)` rather than an error.
pub(super) struct FakeProcFs {
    /// The lock table, or the reason it could not be read.
    table: Result<Vec<LockRecord>, String>,
    /// Per-process descriptor listings, or the reason none could be read.
    entries: BTreeMap<String, Result<DescriptorEntries, String>>,
    /// Per-descriptor identities, or the reason one could not be read.
    identities: BTreeMap<(String, String), Result<LockFileIdentity, String>>,
    /// Per-process stat lines, absent where the pid has no stat.
    stats: BTreeMap<String, ProcessStat>,
}

impl Default for FakeProcFs {
    fn default() -> Self {
        Self {
            table: Ok(Vec::new()),
            entries: BTreeMap::new(),
            identities: BTreeMap::new(),
            stats: BTreeMap::new(),
        }
    }
}

impl FakeProcFs {
    /// Fails the lock-table read with `reason`.
    pub(super) fn with_table_failure(mut self, reason: &str) -> Self {
        self.table = Err(reason.to_owned());
        self
    }

    /// Lists `descriptors` for `pid`, as though the kernel had written them.
    pub(super) fn with_descriptors(mut self, pid: ProcessId, descriptors: Vec<Descriptor>) -> Self {
        self.entries.insert(
            pid.to_string(),
            Ok(DescriptorEntries::new(descriptors, Vec::new())),
        );
        self
    }

    /// Lists `entries` for `pid` exactly as given, unparsed names included.
    pub(super) fn with_entry_listing(mut self, pid: ProcessId, entries: DescriptorEntries) -> Self {
        self.entries.insert(pid.to_string(), Ok(entries));
        self
    }

    /// Fails `pid`'s listing with `reason`.
    pub(super) fn with_listing_failure(mut self, pid: ProcessId, reason: &str) -> Self {
        self.entries.insert(pid.to_string(), Err(reason.to_owned()));
        self
    }

    /// Reports `descriptor` of `pid` as referring to `identity`.
    pub(super) fn with_identity(
        mut self,
        pid: ProcessId,
        descriptor: Descriptor,
        identity: LockFileIdentity,
    ) -> Self {
        self.identities
            .insert((pid.to_string(), descriptor.to_string()), Ok(identity));
        self
    }

    /// Fails the read of `descriptor` in `pid` with `reason`.
    pub(super) fn with_identity_failure(
        mut self,
        pid: ProcessId,
        descriptor: Descriptor,
        reason: &str,
    ) -> Self {
        self.identities.insert(
            (pid.to_string(), descriptor.to_string()),
            Err(reason.to_owned()),
        );
        self
    }

    /// Describes `pid` by `command` and `parent`.
    pub(super) fn with_process(mut self, pid: ProcessId, command: &str, parent: ProcessId) -> Self {
        self.stats.insert(
            pid.to_string(),
            ProcessStat::new(command.to_owned(), parent),
        );
        self
    }
}

impl ProcFs for FakeProcFs {
    fn lock_table(&self) -> Result<Vec<LockRecord>, String> {
        self.table.clone()
    }

    fn descriptor_entries(&self, pid: ProcessId) -> Result<DescriptorEntries, String> {
        // A pid the test did not describe has no descriptor table, which is
        // how the kernel behaves for a process that has exited.
        self.entries
            .get(&pid.to_string())
            .cloned()
            .unwrap_or_else(|| Err(format!("/proc/{pid}/fd: unreadable (no such process)")))
    }

    fn descriptor_identity(
        &self,
        pid: ProcessId,
        descriptor: Descriptor,
    ) -> Result<LockFileIdentity, String> {
        self.identities
            .get(&(pid.to_string(), descriptor.to_string()))
            .cloned()
            .unwrap_or_else(|| Err(format!("/proc/{pid}/fd/{descriptor}: unreadable")))
    }

    fn process_stat(&self, pid: ProcessId) -> Option<ProcessStat> {
        self.stats.get(&pid.to_string()).cloned()
    }
}

/// A clock that advances only when the code under test sleeps.
///
/// `step` is how far each `sleep` moves the clock. A step of zero is the
/// interesting degenerate case: the settle can then never reach its own
/// deadline, so a test can show that the deadline is what ends the loop and
/// that a release observed part-way through still settles.
///
/// Every requested wait is recorded, which is how a test asserts that the
/// settle sleeps *between* probes and not after the answer it is returning.
pub(super) struct FakeClock {
    /// The current instant, advanced by [`FakeClock::sleep`].
    now: Cell<Instant>,
    /// How far each sleep advances the clock.
    step: Duration,
    /// Every duration the code under test asked to wait.
    sleeps: RefCell<Vec<Duration>>,
}

impl FakeClock {
    /// Starts at `start`, advancing by `step` on every sleep.
    pub(super) const fn new(start: Instant, step: Duration) -> Self {
        Self {
            now: Cell::new(start),
            step,
            sleeps: RefCell::new(Vec::new()),
        }
    }

    /// Returns every duration the code under test asked to wait.
    pub(super) fn sleeps(&self) -> Vec<Duration> {
        self.sleeps.borrow().clone()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.now.get()
    }

    fn sleep(&self, duration: Duration) {
        self.sleeps.borrow_mut().push(duration);
        self.now.set(self.now.get() + self.step);
    }
}
