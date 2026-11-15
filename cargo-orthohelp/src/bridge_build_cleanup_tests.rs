//! Deterministic ownership and bounded cleanup tests for Cargo children.

use super::{ChildCleanup, OwnedChild};
use std::cell::Cell;
use std::io;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Copy)]
enum ReapOutcome {
    Reaped,
    Failed,
    Pending,
}

struct ControlledChild {
    kill_attempts: Rc<Cell<usize>>,
    reap_attempts: Rc<Cell<usize>>,
    outcome: ReapOutcome,
    kill_fails: bool,
}

impl ChildCleanup for ControlledChild {
    fn kill(&mut self) -> io::Result<()> {
        self.kill_attempts
            .set(self.kill_attempts.get().saturating_add(1));
        if self.kill_fails {
            Err(io::Error::other("injected termination failure"))
        } else {
            Ok(())
        }
    }

    fn try_reap(&mut self) -> io::Result<bool> {
        self.reap_attempts
            .set(self.reap_attempts.get().saturating_add(1));
        match self.outcome {
            ReapOutcome::Reaped => Ok(true),
            ReapOutcome::Failed => Err(io::Error::other("injected reap failure")),
            ReapOutcome::Pending => Ok(false),
        }
    }
}

fn controlled_child(
    outcome: ReapOutcome,
    kill_fails: bool,
) -> (ControlledChild, Rc<Cell<usize>>, Rc<Cell<usize>>) {
    let kill_attempts = Rc::new(Cell::new(0));
    let reap_attempts = Rc::new(Cell::new(0));
    (
        ControlledChild {
            kill_attempts: Rc::clone(&kill_attempts),
            reap_attempts: Rc::clone(&reap_attempts),
            outcome,
            kill_fails,
        },
        kill_attempts,
        reap_attempts,
    )
}

#[test]
fn bounded_cleanup_records_termination_and_reap_evidence() {
    let (child, kill_attempts, reap_attempts) = controlled_child(ReapOutcome::Reaped, true);
    let mut owned = OwnedChild::new(child);

    let evidence = owned.stop_bounded_with(Duration::from_millis(20), Duration::ZERO);

    assert!(evidence.contains("termination request failed: Other"));
    assert!(evidence.contains("owned Cargo exited"));
    assert_eq!(kill_attempts.get(), 1);
    assert_eq!(reap_attempts.get(), 1);
    drop(owned);
    assert_eq!(kill_attempts.get(), 1);
}

#[test]
fn bounded_cleanup_preserves_reap_failure_evidence() {
    let (child, _kill_attempts, reap_attempts) = controlled_child(ReapOutcome::Failed, false);
    let mut owned = OwnedChild::new(child);

    let evidence = owned.stop_bounded_with(Duration::from_millis(20), Duration::ZERO);

    assert!(evidence.contains("owned Cargo reap failed: Other"));
    assert_eq!(reap_attempts.get(), 1);
}

#[test]
fn bounded_cleanup_reports_expiry_without_waiting_for_the_production_timeout() {
    let (child, _kill_attempts, reap_attempts) = controlled_child(ReapOutcome::Pending, false);
    let mut owned = OwnedChild::new(child);

    let evidence = owned.stop_bounded_with(Duration::from_millis(5), Duration::from_millis(1));

    assert!(evidence.contains("did not exit before bounded cleanup expired"));
    assert!(reap_attempts.get() > 0);
}

#[test]
fn dropping_an_unreaped_child_requests_termination() {
    let (child, kill_attempts, _reap_attempts) = controlled_child(ReapOutcome::Pending, false);

    drop(OwnedChild::new(child));

    assert_eq!(kill_attempts.get(), 1);
}
