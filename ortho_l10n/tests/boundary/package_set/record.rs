//! The completion record that marks one run's packaged set as whole.
//!
//! The packaging lease and the completion record are two different things that
//! happen to share a directory:
//!
//! - the **lease** is an exclusive kernel lock, held only while packaging runs;
//! - the **record** is a small file naming the run whose roots were built.
//!
//! They are separated because they are asked different questions. "Is another
//! process packaging right now" is answered by the lock and nothing else. "Did
//! *this run* already finish" is answered by the record, and it survives the
//! release of the lock by design. Keeping the persistence here means
//! [`super::lease`] is only about acquisition, waiting and the deadline, and
//! every file operation on the record has one home.
//!
//! # The lock is a precondition, not a convention
//!
//! Writing or clearing the record is only correct under the lease: a reader
//! must never see a record describing roots that are half-replaced. Rather than
//! say so in a comment, [`record_complete`] and [`clear_record`] take the
//! [`PackagingLease`] itself. A caller cannot name a lease it has already
//! dropped, the borrow keeps the lock alive across the write, and the directory
//! that is written is the one the lease protects, so "hold a lease and name a
//! different directory" is not expressible.
//!
//! Reading is split by the same distinction. [`read_under_lease`] takes the
//! lease; [`read_unlocked`] is for the arrival check that runs before any lease
//! exists, and it takes the directory directly because it has no proof to
//! offer.

use super::lease::PackagingLease;
use anyhow::{Context, Result};
use cap_std::fs_utf8::Dir;
use std::io;

/// Name of the record naming the run whose roots are materialized.
///
/// The leading dot keeps it out of the way of the archives `cargo package`
/// writes beside it, and the name says what it asserts rather than what it is.
pub(crate) const COMPLETION_FILE_NAME: &str = ".packaging-complete";

/// Record that this run's packaged set is complete.
///
/// Only the successful end of a packaging step may call this: an earlier call
/// would claim roots that do not exist yet, and a later one would be describing
/// a set another process may already have replaced.
///
/// # Errors
///
/// Returns an error when the record cannot be written.
pub(crate) fn record_complete(lease: &PackagingLease, scope: &str) -> Result<()> {
    lease
        .directory()
        .write(COMPLETION_FILE_NAME, scope)
        .context("record that the packaged set is complete")
}

/// Drop the record before the roots are replaced.
///
/// Called under the lease, before the first packaging invocation. Without it a
/// run could record a set, lose a root, and have a later contract in the same
/// run read the record as proof that the set is whole. Removing it first makes
/// the invariant local: the record exists only between a successful pair and
/// the next packaging step, and a failed step leaves none behind.
///
/// A record already absent is the ordinary first-run case and is not an error;
/// any other removal failure is reported, because a record that survives the
/// clear would describe roots about to be replaced.
///
/// # Errors
///
/// Returns an error when the record exists and cannot be removed.
pub(crate) fn clear_record(lease: &PackagingLease) -> Result<()> {
    match lease.directory().remove_file(COMPLETION_FILE_NAME) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("clear the packaging completion record"),
    }
}

/// Whether the record names this run, read while holding the lease.
///
/// # Errors
///
/// Returns an error when the record exists but cannot be read.
pub(crate) fn read_under_lease(lease: &PackagingLease, scope: Option<&str>) -> Result<bool> {
    read_unlocked(lease.directory(), scope)
}

/// Whether the record names this run, read without holding the lease.
///
/// Used for the arrival check only, before a lease is taken. A missing record
/// reads as "not complete" and is not an error: that is the ordinary case of a
/// target nothing has packaged into yet. A record that exists and cannot be
/// read is *reported*, because a permission or handle fault is not evidence
/// that the step has yet to run - treating it as absent would silently package
/// beside a contract that may be about to succeed.
///
/// # Errors
///
/// Returns an error when the record exists but cannot be read.
pub(crate) fn read_unlocked(directory: &Dir, scope: Option<&str>) -> Result<bool> {
    match directory.read_to_string(COMPLETION_FILE_NAME) {
        Ok(record) => Ok(scope.is_some_and(|this_run| record == this_run)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("read the packaging completion record"),
    }
}
