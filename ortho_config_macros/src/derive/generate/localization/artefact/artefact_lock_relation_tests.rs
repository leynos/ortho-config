//! The relation `describe_process` states between a holder and the reader.
//!
//! `relation` has four branches and the report suite reaches only two of them:
//! `this process` and `child of this process` are asserted because a real hold
//! produces them. The other two — a known parent that is neither the reader nor
//! its child, and a holder whose stat line cannot be read — need a process the
//! kernel will not supply on cue, so they were unasserted. A mutant mapping
//! every known parent to `child of this process` passed the whole suite.
//!
//! Split from the holder suite for the file-length cap, and because it pins the
//! wording of a sentence rather than the behaviour of the scan.

use anyhow::Result;

use super::lock_evidence::ProcessId;
use super::lock_fakes::FakeProcFs;
use super::lock_process::describe_process;

/// Verifies a holder unrelated to the reader is called `other`.
///
/// This is the ordinary foreign-holder case from the reader's point of view:
/// the lock is held by a process that is not this one and was not forked by it
/// — a sibling, a service, or a process that outlived its parent. Saying
/// "child of this process" here would point a maintainer at their own test's
/// descendants and away from the real holder.
///
/// The assertion is on the exact relation sentence, not a substring: `other`
/// is a prefix of `other (parent unknown)`, so a `contains` check would pass
/// for the wrong branch.
#[test]
fn a_holder_neither_this_process_nor_its_child_is_other() -> Result<()> {
    use anyhow::ensure;

    let own_pid = ProcessId::new(100);
    let holder = ProcessId::new(200);
    let procfs = FakeProcFs::default().with_process(holder, "some-daemon", ProcessId::new(1));

    let described = describe_process(holder, own_pid, &procfs);

    ensure!(
        described.ends_with(", other"),
        "a holder unrelated to the reader must be called exactly `other`, \
         because its relation is known and is neither of the other two: \
         {described}",
    );
    ensure!(
        !described.contains("child of this process"),
        "a process forked by someone else must not be reported as this \
         process's child: {described}",
    );
    Ok(())
}

/// Verifies an unreadable stat line is reported as an unknown relation.
///
/// A process can exit between the lock record that names it and the read that
/// would describe it, so this is ordinary rather than exceptional. The report
/// must say the relation is *unknown*: a maintainer deciding whether a child
/// inherited the lock needs to know that the question could not be answered,
/// which is a different fact from the process having no parent recorded.
#[test]
fn a_holder_whose_stat_cannot_be_read_has_an_unknown_relation() -> Result<()> {
    use anyhow::ensure;

    let own_pid = ProcessId::new(100);
    // Deliberately no `with_process`: the fake answers `None` for a pid it was
    // not told about, which is what an exited process looks like.
    let vanished = ProcessId::new(300);
    let procfs = FakeProcFs::default();

    let described = describe_process(vanished, own_pid, &procfs);

    ensure!(
        described.contains("<stat unreadable>"),
        "the report must say the stat line could not be read rather than \
         omitting the holder's command silently: {described}",
    );
    ensure!(
        described.ends_with(", other (parent unknown)"),
        "an unreadable stat line means the relation is unknown, and the report \
         must say so in those words rather than claiming `other`: {described}",
    );
    ensure!(
        !described.contains("child of this process"),
        "an unknown relation must not be reported as a known one: {described}",
    );
    Ok(())
}
