//! Cross-process lease over the identifier-artefact fixture's private target.
//!
//! A fixed target directory is shared state: a second `cargo test`, `make test`
//! or nextest invocation for another target can reach the same fixture and run
//! its own `cargo clean -p orthohelp_fixture` inside this process's build or
//! artefact read. nextest's process group and `#[serial]` only coordinate one
//! runner, so the exclusion has to come from the operating system.
//!
//! `cap-std` exposes no file-locking API, so this module is the one place in
//! the identifier-artefact target that uses the standard library's lock. It is
//! accordingly exempted, by a single module-scoped entry in `dylint.toml`, from
//! the capability policy; every other filesystem access in the target keeps
//! using `cap_std` handles. The file it opens is created inside the fixture's
//! own private target, never an ambient path chosen by this module.

use anyhow::{Context, Result};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use std::fs::File;
use std::path::Path;

/// Names the lease file that serializes whole tests over the private target.
///
/// The file lives inside the target so it shares the target's lifetime and its
/// `CACHEDIR.TAG` marking. Its contents are irrelevant: the lease is the
/// kernel-held lock on it, released when the owning handle closes.
const LEASE_FILE_NAME: &str = ".identifier-artefact.lease";

/// A lease held for as long as the test owns the fixture.
///
/// The lock lives exactly as long as this value: dropping it releases the
/// lease, which is why the fixture carries it for the whole test.
pub(super) struct TargetLease {
    /// The locked file. Never read; kept so the kernel lock outlives the call.
    _file: File,
}

/// Open, creating if needed, and exclusively lock the target's lease file.
///
/// Blocks until every other leaseholder has released the target, so a
/// concurrent test process cannot interleave its `cargo clean -p` with this
/// test's build or artefact read.
pub(super) fn acquire(target: &Path) -> Result<TargetLease> {
    let directory = Dir::open_ambient_dir(target, ambient_authority())
        .context("open the identifier-artefact target for its lease")?;
    // `truncate(false)` matters: the lease is the lock on the file, and
    // clearing it would be a spurious write racing with another opener.
    let file = directory
        .open_with(
            LEASE_FILE_NAME,
            OpenOptions::new().create(true).write(true).truncate(false),
        )
        .context("open the identifier-artefact target lease")?
        .into_std();
    file.lock()
        .context("lease the identifier-artefact target")?;
    Ok(TargetLease { _file: file })
}
