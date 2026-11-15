//! Check the spawn guard's disarmed and failed-kill branches.
//!
//! The guard exists for one window: a process-tree wrapper that has already
//! spawned the root child but not yet taken ownership of it. The happy path is
//! covered by every caller of the runner; these cases force the paths where the
//! guard is disarmed, where the child has no wrapper around it, and where the
//! kill it requests is refused, so the evidence it hands the caller is known to
//! be right rather than assumed.
//!
//! A scripted child is used rather than a real one because the standard library
//! answers a kill of an already-reaped child with success, which hides exactly
//! the refusal this module has to report.

use super::{GuardedChild, SpawnGuard};

use process_wrap::std::ChildWrapper;
use std::io;
use std::process::ExitStatus;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A child that records every kill request and can refuse it.
#[derive(Debug)]
struct ScriptedChild {
    killed: Arc<AtomicBool>,
    kill_refused: bool,
}

impl ScriptedChild {
    /// A child that accepts a kill request.
    fn compliant() -> Self {
        Self {
            killed: Arc::new(AtomicBool::new(false)),
            kill_refused: false,
        }
    }

    /// A child that refuses a kill request.
    fn refusing() -> Self {
        Self {
            kill_refused: true,
            ..Self::compliant()
        }
    }
}

impl ChildWrapper for ScriptedChild {
    fn inner(&self) -> &dyn ChildWrapper {
        self
    }

    fn inner_mut(&mut self) -> &mut dyn ChildWrapper {
        self
    }

    fn into_inner(self: Box<Self>) -> Box<dyn ChildWrapper> {
        self
    }

    fn start_kill(&mut self) -> io::Result<()> {
        self.killed.store(true, Ordering::Release);
        if self.kill_refused {
            Err(io::Error::other("scripted kill refusal"))
        } else {
            Ok(())
        }
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Ok(None)
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        Err(io::Error::other("the scripted child has no unbounded wait"))
    }
}

/// Wrap a child in a guarded wrapper sharing the guard's state.
fn guarded(child: Option<Box<dyn ChildWrapper>>, guard: &SpawnGuard) -> GuardedChild {
    GuardedChild {
        child,
        state: Arc::clone(&guard.state),
    }
}

#[test]
fn a_guard_that_never_killed_reports_no_error() {
    let guard = SpawnGuard::new();

    assert!(!guard.kill_attempted());
    assert!(
        guard.kill_error().is_none(),
        "a guard that requested no kill has no error to report"
    );
}

#[test]
fn the_default_guard_is_armed_and_quiet() {
    let guard = SpawnGuard::default();

    assert!(!guard.root_child_seen());
    assert!(!guard.kill_attempted());
}

#[test]
fn a_disarmed_guard_leaves_the_child_alone() {
    let guard = SpawnGuard::new();
    guard.disarm();
    let child = ScriptedChild::compliant();

    drop(guarded(Some(Box::new(child)), &guard));

    assert!(
        !guard.kill_attempted(),
        "a disarmed guard must not touch the child"
    );
}

#[test]
fn a_wrapper_without_a_child_forwards_to_itself() {
    let guard = SpawnGuard::new();
    let mut wrapper = guarded(None, &guard);

    {
        let _forwarded = wrapper.inner_mut();
    }

    assert!(
        wrapper.child.is_none(),
        "the wrapper has no inner child to forward to"
    );
    drop(wrapper);
    assert!(
        guard.kill_error().is_none(),
        "a guard with no child behind it has no kill failure to report"
    );
}

#[test]
fn unwrapping_an_armed_child_kills_it_first() {
    let guard = SpawnGuard::new();
    let child = ScriptedChild::compliant();
    let killed = Arc::clone(&child.killed);

    let unwrapped = Box::new(guarded(Some(Box::new(child)), &guard)).into_inner();

    assert!(
        killed.load(Ordering::Acquire),
        "the guard kills a child it is handing on"
    );
    assert!(guard.kill_attempted());
    drop(unwrapped);
}

#[test]
fn unwrapping_an_empty_wrapper_returns_the_wrapper() {
    let guard = SpawnGuard::new();

    let unwrapped = Box::new(guarded(None, &guard)).into_inner();

    assert!(
        guard.kill_error().is_none(),
        "there is no child to kill behind the wrapper, so nothing failed"
    );
    drop(unwrapped);
}

#[test]
fn a_refused_kill_is_recorded_for_the_caller() {
    let guard = SpawnGuard::new();
    let child = ScriptedChild::refusing();

    drop(guarded(Some(Box::new(child)), &guard));

    assert!(
        guard.kill_attempted(),
        "the guard records that it tried to kill the child"
    );
    let error = guard
        .kill_error()
        .expect("a refused kill is reported to the caller");
    assert!(
        error.contains("scripted kill refusal"),
        "the reported error is the refusal: {error}"
    );
}
