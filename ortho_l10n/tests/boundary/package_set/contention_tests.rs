//! What a waiter sees while another contract holds the packaging lease.
//!
//! [`super::lease_tests`] pins the outcomes of `prepare`, but every one of its
//! cases starts at an edge: one begins with a held lease and a *spent*
//! deadline, so the refusal is reported at once, and another begins with the
//! lease free, so nothing is waited for. Neither drives the polling loop, which
//! is the only mechanism that turns a release into a hand-off. These two cases
//! put a waiter behind a holder that is genuinely packaging and observe it
//! there.
//!
//! The first is the hand-off itself: a waiter that reaches a held lease stays
//! in the wait, and takes the step within the bound once the holder releases
//! it. The second is the saving underneath the hand-off: once the holder
//! records the roots, the same waiter reuses them instead of packaging a step
//! somebody else has finished - and it does so *without* waiting for the
//! release, because `prepare` re-reads the record on every iteration.
//!
//! Both cases hold the lease in the test's own process. That models a second
//! process on both platforms, for two different reasons: Unix locks are held
//! per open file description, so a second open of the lease contends with the
//! first, and Windows denies every other open of the file outright. A change
//! that broke either property would fail the first observation here rather than
//! pass vacuously.

use crate::boundary::package_set::lease::{Prepared, prepare};
use crate::boundary::package_set::record::record_complete;
use crate::boundary::package_set::scratch::{AMPLE_DEADLINE, Scratch, WAKE_UP_BOUND, scratch_root};
use anyhow::{Context, Result, anyhow, ensure};
use rstest::rstest;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// The run identifier these cases record under.
///
/// Not `NEXTEST_RUN_ID`: a real run's value would be visible to the other
/// contracts in the same binary, and one of them could then read this case's
/// scratch target as already packaged.
const THIS_RUN: &str = "lease-contention-this-run";

/// How long a waiter must demonstrably stay in the wait.
///
/// Five times the poll interval, so a waiter that is polling has made several
/// attempts inside this window rather than being observed on its first. The
/// window bounds an observation of *not returning*: a descheduled waiter
/// satisfies it, and so does a waiter that entered `prepare` and has not yet
/// reached its first attempt. Only a waiter that took the lease, or gave up on
/// it, fails it - and the mutation evidence in the plan is what shows the
/// window discriminates the loop rather than mere non-return.
const PROVEN_IN_WAIT: Duration = Duration::from_millis(250);

/// Name the [`Prepared`] variant that came back, for a failure message.
///
/// `Prepared` carries a live lease and so derives no `Debug`; the message only
/// needs to say which of the two answers arrived.
fn describe(prepared: &Prepared) -> &'static str {
    match prepared {
        Prepared::Reused => "Reused",
        Prepared::Owned { .. } => "Owned",
    }
}

/// Wait in a second thread for the packaging lease.
///
/// The thread reports two events and nothing else: that it reached the call,
/// and what the call answered. The first separates "still waiting" from "never
/// started" in the asserting thread, which cannot see either directly. It is
/// sent immediately before `prepare`, so it proves the call was entered rather
/// than that the lease was attempted; that residual gap is closed by the
/// mutation evidence in the plan, where refusing contention on first sight
/// fails these cases, rather than by the timing alone.
///
/// Both sends are best-effort. If the case has already failed and dropped its
/// receivers there is nobody left to report to, and a panic there would replace
/// the real failure with a second one.
fn spawn_waiter(scratch: &Scratch) -> Waiter {
    let (started, reached) = mpsc::channel();
    let (finished, answered) = mpsc::channel();
    let path = scratch.path().to_path_buf();
    let handle = thread::spawn(move || {
        let _signalled = started.send(());
        let prepared = prepare(&path, Some(THIS_RUN), Instant::now() + AMPLE_DEADLINE);
        let _reported = finished.send(prepared);
    });
    Waiter {
        handle,
        reached,
        answered,
    }
}

/// What one fixed-length look at a waiter saw.
///
/// The outer result is the look: `Err(Timeout)` means the waiter was still
/// waiting for the whole window. The inner result is the waiter's own answer
/// when it did arrive, so a case can report whichever of the two happened.
type Observation = Result<Result<Prepared>, mpsc::RecvTimeoutError>;

/// A waiting contract, and the two channels it reports on.
struct Waiter {
    /// Joined before an assertion can fail, so a failing case never leaves a
    /// thread spinning behind the lease its own case still holds.
    handle: thread::JoinHandle<()>,
    /// Receives once, when the waiter reaches the call.
    reached: mpsc::Receiver<()>,
    /// Receives once, when the call answers.
    answered: mpsc::Receiver<Result<Prepared>>,
}

impl Waiter {
    /// Require the waiter to have reached the call.
    ///
    /// # Errors
    ///
    /// Returns an error when the thread did not start within the bound.
    fn require_started(&self) -> Result<()> {
        self.reached.recv_timeout(WAKE_UP_BOUND).with_context(|| {
            format!("the waiting contract did not reach the lease within {WAKE_UP_BOUND:?}")
        })
    }

    /// Observe the waiter for a window, without releasing anything.
    ///
    /// The outer result is the observation: `Err(Timeout)` is the waiter still
    /// waiting, which is what the callers require. The inner result is the
    /// waiter's own answer, if it arrived, and is passed back untouched.
    fn observe_wait(&self) -> Observation {
        self.answered.recv_timeout(PROVEN_IN_WAIT)
    }

    /// The waiter's answer after whatever released it.
    ///
    /// Two failures are reported differently: no answer inside the bound is
    /// this harness's failure to observe a hand-off, and an `Err` answer is the
    /// waiter's own report, passed through with its context intact.
    ///
    /// # Errors
    ///
    /// Returns an error when no answer arrives within the hand-off bound, or
    /// when the waiter's own call failed.
    fn require_answer(&self) -> Result<Prepared> {
        self.answered.recv_timeout(WAKE_UP_BOUND).with_context(|| {
            format!("the waiting contract did not answer within {WAKE_UP_BOUND:?} of release")
        })?
    }

    /// Join the thread, reporting a panic in it as a failed case.
    ///
    /// # Errors
    ///
    /// Returns an error when the thread panicked.
    fn join(self) -> Result<()> {
        self.handle
            .join()
            .map_err(|_| anyhow!("the waiting contract's thread panicked"))
    }
}

/// A waiter behind a holder keeps waiting, then takes the step on release.
///
/// The two halves of the lease's behaviour have to hold together: while a
/// holder owns it, a second contract must not take it, and once the holder
/// releases it, that contract must not sleep past the hand-off. This case
/// starts a waiter at a lease held in this process, proves it is still waiting,
/// releases the holder, and requires the waiter to own the step promptly.
#[rstest]
fn a_waiter_behind_a_holder_takes_the_step_once_it_is_released(
    scratch_root: Result<Scratch>,
) -> Result<()> {
    let scratch = scratch_root?;
    let holder = scratch.take(Some(THIS_RUN))?;
    let waiter = spawn_waiter(&scratch);
    waiter.require_started()?;

    let idle = waiter.observe_wait();
    let released = Instant::now();
    drop(holder);
    let handed_off = waiter.require_answer();
    let waited = released.elapsed();
    waiter.join()?;

    ensure!(
        matches!(idle, Err(mpsc::RecvTimeoutError::Timeout)),
        "a contract that reached a lease held by another contract was answered \
         within {PROVEN_IN_WAIT:?}, so it took the lease or gave up on it \
         instead of waiting: two processes would package the same target side \
         by side"
    );
    let prepared = handed_off?;
    ensure!(
        matches!(prepared, Prepared::Owned { .. }),
        "a waiter that was first in the queue for a released lease was told the \
         roots were already complete, but nothing had recorded them: {}",
        describe(&prepared),
    );
    ensure!(
        waited <= WAKE_UP_BOUND,
        "the hand-off after a release took {waited:?}, past the \
         {WAKE_UP_BOUND:?} a waiter may add; the polling loop is not handing off"
    );
    Ok(())
}

/// A waiter reuses the roots the holder records, without waiting to acquire.
///
/// This is the saving that collapses a run's packaging from one pair per
/// contract to one pair: a contract that arrived before the roots existed must
/// not package them once the contract ahead of it has. It is the cross-process
/// case a per-process cache cannot provide.
///
/// The record is honoured *while the holder still owns the lease*, and that is
/// deliberate rather than incidental. `prepare` reads the record on every
/// iteration of its wait, so a waiter takes the answer as soon as it appears;
/// the assertion is therefore made before the holder is released. A `prepare`
/// that read the record only on arrival would sit in the wait here and fail on
/// the bound, which is what makes this case about the re-read rather than about
/// the release.
#[rstest]
fn a_waiter_reuses_the_roots_the_holder_records_while_it_waits(
    scratch_root: Result<Scratch>,
) -> Result<()> {
    let scratch = scratch_root?;
    let holder = scratch.take(Some(THIS_RUN))?;
    let waiter = spawn_waiter(&scratch);
    waiter.require_started()?;

    let still_waiting = waiter.observe_wait();
    record_complete(&holder, THIS_RUN)?;
    let reused = waiter.require_answer();
    drop(holder);
    waiter.join()?;

    ensure!(
        matches!(still_waiting, Err(mpsc::RecvTimeoutError::Timeout)),
        "the waiter was answered within {PROVEN_IN_WAIT:?} of reaching the \
         lease, before the holder had recorded the roots; it did not wait at \
         all, so this case observed a first arrival rather than a waiter"
    );
    let prepared = reused?;
    ensure!(
        matches!(prepared, Prepared::Reused),
        "a waiter whose holder recorded the roots while it waited was told to \
         package them again (answer: {}), so it repeats work the holder has \
         already finished",
        describe(&prepared),
    );
    Ok(())
}
