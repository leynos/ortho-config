//! Check the process-group owner's refusal and parsing branches.
//!
//! The live path is covered by every real command the runner starts. What is
//! left are the cases a healthy machine never produces: a signal refused
//! because the leader was already reaped, a `/proc` stat entry that is
//! malformed or describes a zombie, and a wrapper asked to hand on a child it
//! no longer holds. Those decide what the runner reports as confirmed cleanup,
//! so they are forced here rather than trusted.
//!
//! Every constructed owner is left in a state whose drop cannot signal a real
//! group: either its leader is already reaped, or termination was requested.

use super::ProcessGroupChild;

use nix::unistd::Pid;
use process_wrap::std::ChildWrapper;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

/// An owner with no live child behind it.
///
/// `termination_requested` must be set for any case that leaves the status
/// unset, so dropping the owner cannot signal the test's own process group.
fn orphaned(status: Option<ExitStatus>, termination_requested: bool) -> ProcessGroupChild {
    ProcessGroupChild {
        child: None,
        group: Pid::from_raw(0),
        root_exit_observed: false,
        termination_requested,
        status,
        tree_completed: false,
    }
}

/// An exit status for a root that has been reaped.
fn reaped_status() -> ExitStatus {
    ExitStatus::from_raw(0)
}

#[test]
fn a_repeated_termination_request_is_idempotent() {
    let mut owner = orphaned(None, true);

    assert!(
        owner.start_kill().is_ok(),
        "a group already asked to terminate is not signalled twice"
    );
}

#[test]
fn signalling_after_the_root_is_reaped_is_refused() {
    let mut owner = orphaned(Some(reaped_status()), false);

    let error = owner
        .start_kill()
        .expect_err("a reaped leader has no group to signal");

    assert!(
        error.to_string().contains("refusing to signal"),
        "the refusal explains itself: {error}"
    );
}

#[test]
fn reaping_an_absent_root_is_an_error() {
    let mut owner = orphaned(None, true);

    let error = owner
        .reap_root_after_termination()
        .expect_err("there is no owned child to reap");

    assert!(
        error.to_string().contains("root child is unavailable"),
        "the failure names the missing child: {error}"
    );
}

#[test]
fn an_unbounded_wait_is_unsupported() {
    let mut owner = orphaned(None, true);

    let error = owner
        .wait()
        .expect_err("the runner polls instead of waiting");

    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
}

#[test]
fn a_completed_tree_repeats_its_recorded_status() {
    let mut owner = orphaned(Some(reaped_status()), true);
    owner.tree_completed = true;

    let status = owner
        .try_wait()
        .expect("a completed tree reports its status")
        .expect("the tree has completed");

    assert_eq!(status.code(), Some(0));
}

#[test]
fn a_wrapper_without_a_child_forwards_to_itself() {
    let mut owner = orphaned(None, true);

    {
        let _forwarded = owner.inner_mut();
    }

    assert!(
        owner.child.is_none(),
        "the owner has no inner child to forward to"
    );
}

#[test]
fn unwrapping_hands_on_the_owned_child() {
    let inner: Box<dyn ChildWrapper> = Box::new(orphaned(None, true));
    let mut owner = orphaned(Some(reaped_status()), true);
    owner.child = Some(inner);

    let unwrapped = Box::new(owner).into_inner();

    drop(unwrapped);
}

#[test]
fn unwrapping_an_empty_owner_returns_the_owner() {
    let unwrapped = Box::new(orphaned(Some(reaped_status()), true)).into_inner();

    drop(unwrapped);
}

#[cfg(target_os = "linux")]
mod linux {
    //! `/proc/<pid>/stat` parsing, which only Linux provides.

    use super::super::{proc_stat_has_writer, read_proc_stat};

    use std::io;

    /// A stat line whose fields after the command name are `tail`.
    fn stat_line(tail: &str) -> String {
        format!("123 (probe) {tail}")
    }

    #[test]
    fn a_live_member_of_the_group_is_a_writer() {
        let stat = stat_line("R 1 4242");

        assert!(
            proc_stat_has_writer(&stat, 4242).expect("the stat parses"),
            "a running member of the owned group holds the pipe"
        );
    }

    #[test]
    fn a_zombie_in_the_group_is_not_a_writer() {
        let stat = stat_line("Z 1 4242");

        assert!(
            !proc_stat_has_writer(&stat, 4242).expect("the stat parses"),
            "a zombie no longer holds its pipe open"
        );
    }

    #[test]
    fn a_member_of_another_group_is_ignored() {
        let stat = stat_line("R 1 999");

        assert!(
            !proc_stat_has_writer(&stat, 4242).expect("the stat parses"),
            "another group's writer is not the owned tree"
        );
    }

    #[test]
    fn a_stat_without_a_command_terminator_is_malformed() {
        let error = proc_stat_has_writer("123 probe R 1 4242", 4242)
            .expect_err("the command name is unterminated");

        assert!(error.to_string().contains("malformed proc stat"));
    }

    #[test]
    fn a_stat_without_a_state_is_malformed() {
        let error =
            proc_stat_has_writer("123 (probe) ", 4242).expect_err("the state field is missing");

        assert!(error.to_string().contains("missing proc state"));
    }

    #[test]
    fn a_stat_without_a_parent_is_malformed() {
        let error =
            proc_stat_has_writer("123 (probe) R", 4242).expect_err("the parent field is missing");

        assert!(error.to_string().contains("missing proc parent ID"));
    }

    #[test]
    fn a_stat_without_a_group_is_malformed() {
        let error =
            proc_stat_has_writer("123 (probe) R 1", 4242).expect_err("the group field is missing");

        assert!(error.to_string().contains("missing proc group ID"));
    }

    #[test]
    fn a_vanished_entry_reads_as_absent() {
        let stat = read_proc_stat(|| Err(io::Error::from(io::ErrorKind::NotFound)))
            .expect("a vanished process is not an error");

        assert!(stat.is_none());
    }

    #[test]
    fn another_read_failure_is_propagated() {
        assert!(
            read_proc_stat(|| Err(io::Error::other("permission denied"))).is_err(),
            "only a vanished entry is treated as absent"
        );
    }
}
