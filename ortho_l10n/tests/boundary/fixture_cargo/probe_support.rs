//! Scaffolding for probes that re-run this test binary as their own child.
//!
//! The boundary regressions need a real timed-out child and real descendants,
//! and the only portable child this binary can rely on is itself. Each probe is
//! an ignored test selected by name, so an ordinary run never waits for one.
//!
//! A descendant's lifetime is observable without heartbeats: it takes an
//! exclusive lock on a file in a scratch directory shared with the parent, and
//! the kernel releases that lock when the process dies. A parent that can take
//! the lock therefore knows the descendant is gone, and one that cannot knows
//! it is still alive. [`descendant_lock`] owns that observation, because the
//! standard library is the only portable locking API and it needs an explicitly
//! reviewed exemption from the capability policy. The scratch directory is also
//! how a parent tells a descendant when it may exit, so a probe that survives a
//! kill does not linger.

use anyhow::{Context, Result};
use cap_std::{ambient_authority, fs::Dir};
use process_wrap::std::ChildWrapper;
use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

#[path = "descendant_lock.rs"]
pub(super) mod descendant_lock;

/// How long a probe or test waits for a child to report that it is ready.
///
/// Generous on purpose: on a host running a hundred test binaries at once the
/// child may take tens of seconds to reach its first line, and waiting for
/// readiness is exactly how the regressions keep that delay out of the
/// deadline they are measuring.
pub(super) const DESCENDANT_STARTUP: Duration = Duration::from_secs(120);

/// How long a test waits for a killed or released child to release its lock.
pub(super) const DESCENDANT_TEARDOWN: Duration = Duration::from_secs(30);

/// Environment variable naming the scratch directory shared with a descendant.
pub(super) const SCRATCH_VARIABLE: &str = "PROBE_DESCENDANT_SCRATCH";

/// Name of the file a child writes once it has reached the state its test waits for.
///
/// A descendant writes it after taking its lock; a probe with no descendant
/// writes it after flushing the output the test asserts about.
pub(super) const READY_FILE_NAME: &str = "probe.ready";

/// Name of the file a test writes to release a surviving descendant.
pub(super) const RELEASE_FILE_NAME: &str = "descendant.release";

/// How often a probe polls for a marker file or a released lock.
const PROBE_POLL: Duration = Duration::from_millis(10);

/// Build the command that re-runs this binary as a named probe.
pub(super) fn probe_command(filter: &str) -> Result<Command> {
    let executable = std::env::current_exe().context("the test binary should have a path")?;
    let mut command = Command::new(executable);
    // `--nocapture` matters: without it libtest buffers each test's output and
    // the child would print nothing to the pipes this test is asserting about.
    command.args([
        "--ignored",
        "--quiet",
        "--nocapture",
        "--test-threads=1",
        filter,
    ]);
    Ok(command)
}

/// Build a probe command that also points its descendant at the scratch directory.
pub(super) fn probe_command_with(filter: &str, scratch: &Path) -> Result<Command> {
    let mut command = probe_command(filter)?;
    command.env(SCRATCH_VARIABLE, scratch);
    Ok(command)
}

/// Start a descendant probe that inherits this process's streams.
pub(super) fn spawn_probe(filter: &str) -> Result<Child> {
    probe_command(filter)?
        .spawn()
        .with_context(|| format!("start the {filter} descendant"))
}

/// Spawn a probe as the root of a process tree the deadline runner owns.
///
/// The runner's deadline is measured from the later call to
/// [`super::supervise`], so a loaded host's process-startup delay is never
/// charged to the probe. The caller waits for the probe's readiness marker in
/// between.
pub(super) fn spawn_owned_probe(filter: &str, scratch: &Path) -> Result<Box<dyn ChildWrapper>> {
    super::spawn_owned(probe_command_with(filter, scratch)?)
}

/// Wait until the descendant has taken its lock and says so.
pub(super) fn wait_for_descendant() -> Result<()> {
    wait_for_ready(&scratch_directory()?)
}

/// Wait until a child using `scratch` reports that it has reached its setup point.
pub(super) fn wait_for_ready_in(scratch: &Path) -> Result<()> {
    wait_for_ready(&open_scratch(scratch)?)
}

/// Wait until the child with this scratch handle reports readiness.
fn wait_for_ready(scratch: &Dir) -> Result<()> {
    wait_until(DESCENDANT_STARTUP, "the probe to become ready", || {
        Ok(scratch.exists(READY_FILE_NAME))
    })
}

/// Announce readiness from a probe that has no descendant to do it.
pub(super) fn announce_ready(scratch: &Dir) -> Result<()> {
    scratch
        .write(READY_FILE_NAME, b"ready")
        .context("announce the probe is ready")
}

/// Open the scratch directory named by the probe command.
pub(super) fn scratch_directory() -> Result<Dir> {
    let path = std::env::var_os(SCRATCH_VARIABLE)
        .with_context(|| format!("{SCRATCH_VARIABLE} should name a scratch directory"))?;
    open_scratch(Path::new(&path))
}

/// Open `path` as a scratch directory handle.
fn open_scratch(path: &Path) -> Result<Dir> {
    Dir::open_ambient_dir(path, ambient_authority())
        .with_context(|| format!("open the probe scratch directory {}", path.display()))
}

/// Poll `condition` until it holds or `limit` elapses.
pub(super) fn wait_until(
    limit: Duration,
    description: &str,
    mut condition: impl FnMut() -> Result<bool>,
) -> Result<()> {
    let deadline = Instant::now() + limit;
    while !condition()? {
        ensure_within(deadline, description)?;
        std::thread::sleep(PROBE_POLL);
    }
    Ok(())
}

/// Fail once `deadline` has passed while waiting for `description`.
fn ensure_within(deadline: Instant, description: &str) -> Result<()> {
    if Instant::now() >= deadline {
        anyhow::bail!("timed out waiting for {description}");
    }
    Ok(())
}

/// Tell a surviving descendant that it may exit.
pub(super) fn release_holder(scratch: &Path) -> Result<()> {
    open_scratch(scratch)?
        .write(RELEASE_FILE_NAME, b"release")
        .context("write the descendant release marker")
}
