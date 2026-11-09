//! Two-process regression for the identifier-artefact target lease.
//!
//! The identifier test drives one stable target directory. nextest's
//! `boundary_packaging` group and `#[serial]` only coordinate tests inside one
//! runner; a second `cargo test` process on the same checkout can still run
//! `cargo clean -p orthohelp_fixture` between this test's build and its artefact
//! read. [`identifier_artefact_e2e_support::fixture_cargo`] holds a
//! cross-process file lease for the whole test to prevent that.
//!
//! This module proves the lease does its job, and that the proof is not
//! vacuous. The parent holds the lease, builds an artefact, and starts a second
//! process from this same test binary. That process discovers the fixture
//! (which is safe), announces it has located the target, and then blocks
//! acquiring the lease. The parent confirms the second process has not
//! acquired the lease while the parent holds it, and still cannot observe its
//! own artefact changing. Only then does the parent release the lease; the
//! second process must then see the parent's artefact intact, clean, rebuild
//! it to the same content, and say so. The measured time the second process
//! spent waiting on the lease is asserted to be non-trivial, so removing the
//! lease turns this test red rather than letting it pass by accident.

use super::identifier_artefact_e2e_support::{
    artefact, build_fixture, clean_fixture, fixture_cargo, locate_fixture,
};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
#[cfg(windows)]
use process_wrap::std::JobObject;
#[cfg(unix)]
use process_wrap::std::ProcessGroup;
use process_wrap::std::{ChildWrapper, CommandWrap};
use serial_test::serial;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Environment variable naming the scratch directory shared with the child.
const SCRATCH_VARIABLE: &str = "IDENTIFIER_LEASE_SCRATCH";

/// How long the second process may take to discover and announce the target.
///
/// Discovery runs `cargo metadata`, which can queue behind a busy package-cache
/// lock on a shared host, so this is deliberately much longer than the work
/// itself.
const HELPER_STARTUP: Duration = Duration::from_secs(120);

/// How long the parent keeps its lease after the second process is ready.
///
/// The lease cannot be acquired during this window, so the second process's
/// measured wait must cover it. See [`MIN_LEASE_WAIT`].
const LEASE_HOLD: Duration = Duration::from_secs(2);

/// The least lease wait that proves the second process really blocked.
///
/// Without a lease the acquisition is a single uncontended syscall, so a wait
/// near zero means the regression would have passed vacuously. Scheduling noise
/// is allowed for, but not enough to hide a missing lock.
const MIN_LEASE_WAIT: Duration = Duration::from_millis(500);

/// How long the second process may take to clean, rebuild, and report once the
/// lease is released.
const HELPER_DEADLINE: Duration = Duration::from_secs(900);

/// How often the parent polls the second process and its marker files.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// How long the parent waits for a killed second process to disappear.
const TEARDOWN: Duration = Duration::from_secs(5);

/// The child half: wait for the lease, prove the artefact survived, rebuild it.
///
/// Ignored so an ordinary run never starts it; the test below selects it by
/// exact name when it re-runs this binary as its own child.
#[test]
#[ignore = "driven as a child process by a_second_process_cannot_clean_a_leased_artefact"]
fn lease_helper() -> Result<()> {
    let scratch = required_scratch()?;
    let baseline = scratch
        .read_to_string("baseline.json")
        .context("read the parent's artefact baseline")?;
    let out_dir_text = scratch
        .read_to_string("out_dir.txt")
        .context("read the parent's build-script directory")?;
    let out_dir = PathBuf::from(out_dir_text.trim());
    let location = locate_fixture()?;
    scratch
        .write("located", b"located")
        .context("announce the located target")?;
    let started = Instant::now();
    let cargo = location.lease()?;
    let lease_wait = started.elapsed();
    // The parent released the lease only after confirming that this process had
    // not acquired it, so the artefact it built must still be there, unchanged.
    let inherited = artefact(&out_dir)?;
    ensure!(
        inherited == baseline,
        "the parent's artefact changed before the lease was acquired; the lease \
         did not exclude the parent's build from this process",
    );
    scratch
        .write("acquired", lease_wait.as_millis().to_string())
        .context("announce the acquired lease")?;
    clean_fixture(&cargo)?;
    let rebuilt = build_fixture(true, &cargo, None)?;
    ensure!(
        artefact(rebuilt.out_dir())? == baseline,
        "the rebuild should reproduce the leased artefact",
    );
    scratch
        .write(
            "report.txt",
            format!(
                "lease_wait_ms={}\nparent_artefact=intact\nrebuilt_out_dir={}\n",
                lease_wait.as_millis(),
                rebuilt.out_dir().display(),
            ),
        )
        .context("write the lease-regression report")?;
    Ok(())
}

/// A second process must wait for the lease instead of cleaning under the test.
#[test]
#[serial]
fn a_second_process_cannot_clean_a_leased_artefact() -> Result<()> {
    let cargo = fixture_cargo()?;
    clean_fixture(&cargo)?;
    let out_dir = build_fixture(true, &cargo, None)?.out_dir().to_path_buf();
    let baseline = artefact(&out_dir)?;
    let scratch = TempDir::new().context("create the lease-regression scratch directory")?;
    let scratch_directory = Dir::open_ambient_dir(scratch.path(), ambient_authority())
        .context("open the lease-regression scratch directory")?;
    scratch_directory
        .write("baseline.json", &baseline)
        .context("write the artefact baseline")?;
    scratch_directory
        .write("out_dir.txt", out_dir.display().to_string())
        .context("write the build-script directory")?;

    let mut helper = Helper::spawn(scratch.path())?;
    wait_for_marker(&mut helper, &scratch_directory, "located", HELPER_STARTUP)?;
    // The marker says the second process has located the target; from here it
    // should block on the lease this process holds.
    std::thread::sleep(LEASE_HOLD);
    ensure!(
        !scratch_directory.exists("acquired"),
        "a second test process acquired the target lease while this process held it",
    );
    ensure!(
        artefact(&out_dir)? == baseline,
        "this process's artefact changed while it held the target lease",
    );

    drop(cargo);
    let status = helper
        .await_exit(HELPER_DEADLINE)
        .context("the second test process did not finish after the lease was released")?;
    ensure!(
        status.success(),
        "the second test process failed with {status:?}; its output is above and \
         its scratch directory is {}",
        scratch.path().display(),
    );
    let report = scratch_directory
        .read_to_string("report.txt")
        .context("the second test process left no report")?;
    ensure!(
        report.contains("parent_artefact=intact"),
        "the second test process did not confirm the leased artefact survived:\n{report}",
    );
    let lease_wait = report_field(&report, "lease_wait_ms")?
        .parse::<u128>()
        .context("parse the second process's lease wait")?;
    ensure!(
        lease_wait >= MIN_LEASE_WAIT.as_millis(),
        "the second test process waited only {lease_wait} ms for the lease, so it \
         did not block on one held by this process",
    );
    // Deliberately no second artefact read here. The helper checked the rebuilt
    // content while it still held the lease; this process released its own at
    // `drop(cargo)` and the helper's ended when it exited, so a concurrent test
    // process may legitimately acquire the lease now and clean the output
    // directory. Reading it here would race that process and fail a test whose
    // lease behaved correctly. `parent_artefact=intact` above is the helper's
    // in-lease evidence that the rebuild reproduced the artefact.
    Ok(())
}

/// The second process, owned so a failure cannot leave it running.
struct Helper {
    child: Box<dyn ChildWrapper>,
    /// Set once the child has been reaped, so teardown does not signal a
    /// process group whose leader's identifier the kernel may have reused.
    status: Option<ExitStatus>,
}

impl Helper {
    /// Start the second process as the root of a tree this test can kill.
    fn spawn(scratch: &Path) -> Result<Self> {
        let executable = std::env::current_exe().context("the test binary should have a path")?;
        let mut command = Command::new(executable);
        command
            .args([
                "--ignored",
                "--exact",
                "--quiet",
                "--nocapture",
                "--test-threads=1",
                "identifier_artefact_lease_tests::lease_helper",
            ])
            .env(SCRATCH_VARIABLE, scratch);
        let mut wrapped = CommandWrap::from(command);
        #[cfg(unix)]
        wrapped.wrap(ProcessGroup::leader());
        #[cfg(windows)]
        wrapped.wrap(JobObject);
        let child = wrapped.spawn().context("start the second test process")?;
        Ok(Self {
            child,
            status: None,
        })
    }

    /// Reap the child if it has exited, remembering the result.
    fn try_status(&mut self) -> Result<Option<ExitStatus>> {
        if self.status.is_none() {
            self.status = self.child.try_wait()?;
        }
        Ok(self.status)
    }

    /// Wait for the child, refusing to wait unboundedly for a wedged one.
    fn await_exit(&mut self, limit: Duration) -> Result<ExitStatus> {
        let deadline = Instant::now() + limit;
        loop {
            if let Some(status) = self.try_status()? {
                return Ok(status);
            }
            ensure!(
                Instant::now() < deadline,
                "the second test process did not exit within {limit:?}",
            );
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        if self.status.is_some() {
            return;
        }
        let _kill = self.child.start_kill();
        let deadline = Instant::now() + TEARDOWN;
        while Instant::now() < deadline && self.child.try_wait().is_ok_and(|s| s.is_none()) {
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

/// Wait for the second process to write `marker`, failing if it exits first.
fn wait_for_marker(
    helper: &mut Helper,
    scratch: &Dir,
    marker: &str,
    limit: Duration,
) -> Result<()> {
    let deadline = Instant::now() + limit;
    loop {
        ensure!(
            helper.try_status()?.is_none(),
            "the second test process exited before writing the `{marker}` marker",
        );
        if scratch.exists(marker) {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "timed out waiting for the second test process to write `{marker}`",
        );
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Read one `name=value` line from the second process's report.
fn report_field<'a>(report: &'a str, name: &str) -> Result<&'a str> {
    report
        .lines()
        .find_map(|line| {
            line.strip_prefix(name)
                .and_then(|rest| rest.strip_prefix('='))
        })
        .with_context(|| format!("the second test process's report omitted `{name}`"))
}

/// Open the scratch directory named by the parent.
fn required_scratch() -> Result<Dir> {
    let path = std::env::var_os(SCRATCH_VARIABLE)
        .with_context(|| format!("{SCRATCH_VARIABLE} should name the scratch directory"))?;
    Dir::open_ambient_dir(Path::new(&path), ambient_authority())
        .with_context(|| format!("open the scratch directory {}", Path::new(&path).display()))
}
