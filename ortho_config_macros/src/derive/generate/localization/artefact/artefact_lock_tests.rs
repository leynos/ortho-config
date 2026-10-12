//! Cross-process regression tests for artefact publication locking.
//!
//! `OUT_DIR` is package-scoped, so a package with several targets can have
//! several compiler processes publishing into one artefact directory. These
//! tests exercise real file locks between real processes rather than
//! in-process threads, because an in-process mutex would satisfy a
//! thread-based test while leaving the compiler-process race intact.
//!
//! The exclusivity test is deliberately not built on wall-clock thresholds.
//! A helper publishes a sentinel while it holds the lock, and the contender
//! reads that sentinel only after acquiring the lock itself. A slow or loaded
//! machine changes how long the contender blocks, never what the contender
//! observes, so the assertion stays meaningful under coverage-instrumented CI.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use syn::Ident;

use super::artefact_lock::lock;
use super::publish;
use super::*;
use crate::derive::generate::localization::{CommandIds, FluentMessageId, LocalizationBase};

/// Names the environment variable that turns this test binary into a helper.
const LOCK_HELPER_ENV: &str = "ORTHO_CONFIG_ARTEFACT_LOCK_HELPER";

/// Names the environment variable carrying the helper's mode.
const LOCK_MODE_ENV: &str = "ORTHO_CONFIG_ARTEFACT_LOCK_MODE";

/// Names the environment variable carrying the helper's artefact root.
const LOCK_ROOT_ENV: &str = "ORTHO_CONFIG_ARTEFACT_LOCK_ROOT";

/// Names the environment variable selecting a publishing helper's variant.
///
/// Separate publishers must differ in both fragment filename and recorded
/// crate, so the merge sees two distinct contributions rather than one
/// fragment rewritten twice.
const LOCK_VARIANT_ENV: &str = "ORTHO_CONFIG_ARTEFACT_LOCK_VARIANT";

/// Names the crate a variant helper records against its entries.
const VARIANT_CRATES: [&str; 2] = ["alpha", "beta"];

/// Filters libtest down to the single helper entry point.
///
/// The extra `pub(super)` sibling modules under `artefact` also match the
/// `localization::artefact` filter, so without the full path the child would
/// run the whole fragment and renderer suites as well as the helper.
const LOCK_HELPER_FILTER: &str =
    "derive::generate::localization::artefact::lock_tests::lock_helper_entry_point";

/// Records the state a holder leaves in the artefacts directory.
const SENTINEL_FILE: &str = "lock-sentinel.txt";

/// Names the sentinel value written while the holder still owns the lock.
const SENTINEL_ACQUIRED: &str = "acquired";

/// Names the sentinel value written just before the holder releases the lock.
const SENTINEL_RELEASED: &str = "released";

/// Bounds how long a helper holds the lock before releasing it.
const HOLD_PERIOD: Duration = Duration::from_millis(900);

/// Bounds how long the parent waits for the holder to take the lock.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(20);

/// How long the parent sleeps between sentinel polls.
const POLL_INTERVAL: Duration = Duration::from_millis(25);

static TEMP_ROOT_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Owns an isolated temporary directory used by one lock regression test.
struct TempRoot(PathBuf);

impl TempRoot {
    /// Creates a unique directory below the system temporary directory.
    fn new() -> Result<Self> {
        let sequence = TEMP_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ortho-config-artefact-lock-tests-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).context("create isolated artefact lock test root")?;
        Ok(Self(path))
    }

    /// Returns the root directory available to the current test.
    fn path(&self) -> &Path {
        &self.0
    }

    /// Reads the sentinel, treating a missing file as "not yet written".
    fn sentinel(&self) -> Option<String> {
        fs::read_to_string(self.path().join(SENTINEL_FILE)).ok()
    }

    /// Waits until the holder records that it owns the lock.
    ///
    /// Ordering the two children this way is what makes the exclusivity
    /// assertion meaningful: the contender must arrive while the holder is
    /// still inside its hold window, not before the holder has started.
    fn wait_for_holder(&self) -> Result<()> {
        let deadline = Instant::now() + ACQUIRE_TIMEOUT;
        while Instant::now() < deadline {
            if self.sentinel().as_deref() == Some(SENTINEL_ACQUIRED) {
                return Ok(());
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        Err(anyhow::anyhow!(
            "holder did not acquire the lock within {ACQUIRE_TIMEOUT:?}"
        ))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

/// Spawns this test binary in helper mode against one artefact root.
///
/// The child re-enters through the helper's own test name, so the helper runs
/// in a genuinely separate process and therefore contends on a genuinely
/// separate lock handle. The mode travels through the environment rather than
/// argv: libtest consumes any extra positional argument as an additional name
/// filter, which silently selects zero tests and still exits successfully.
fn spawn_helper(root: &Path, mode: &str) -> Result<Child> {
    spawn_variant(root, mode, None)
}

/// Spawns a helper, optionally recording a variant crate against its entries.
///
/// Variant helpers exist for the concurrent-publication test, where the two
/// children must contribute distinguishable entries and therefore distinct
/// fragment files.
fn spawn_variant(root: &Path, mode: &str, label: Option<&str>) -> Result<Child> {
    let executable = std::env::current_exe().context("locate the test executable")?;
    let mut command = Command::new(executable);
    command
        .args(["--exact", "--nocapture", LOCK_HELPER_FILTER])
        .env(LOCK_HELPER_ENV, "1")
        .env(LOCK_MODE_ENV, mode)
        .env(LOCK_ROOT_ENV, root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(variant) = label {
        command.env(LOCK_VARIANT_ENV, variant);
    }
    command
        .spawn()
        .context("spawn artefact lock helper process")
}

/// Waits for a helper child and reports the exit status with its diagnostics.
fn wait_for(child: Child, label: &str) -> Result<()> {
    let output = child
        .wait_with_output()
        .with_context(|| format!("wait for {label}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        output.status.success(),
        "{label} must succeed; stdout: {stdout}; stderr: {stderr}"
    );
    // A filtered-out helper still exits successfully, so require proof that the
    // helper body actually ran rather than trusting the exit status alone.
    ensure!(
        stdout.contains("1 passed"),
        "{label} must have run the helper body; stdout: {stdout}",
    );
    Ok(())
}

/// Verifies the lock excludes a second process for as long as it is held.
///
/// The holder advertises ownership through the sentinel while still holding
/// the lock. A contender that acquires the lock must therefore observe the
/// released value; observing "acquired" proves the two processes overlapped.
#[test]
fn artefact_lock_is_exclusive_across_processes() -> Result<()> {
    if std::env::var_os(LOCK_HELPER_ENV).is_some() {
        return Ok(());
    }
    let root = TempRoot::new()?;
    let holder = spawn_helper(root.path(), "hold")?;
    root.wait_for_holder()?;

    let contender = spawn_helper(root.path(), "contend")?;
    wait_for(contender, "contending helper")?;
    wait_for(holder, "holding helper")?;
    ensure!(
        root.sentinel().as_deref() == Some(SENTINEL_RELEASED),
        "holder must record its release before exiting",
    );
    Ok(())
}

/// Verifies the lock is released when a publication path returns.
///
/// The reader uses a non-blocking acquisition, so a lock left held fails the
/// assertion immediately instead of hanging the suite.
#[test]
fn artefact_lock_is_released_after_publication_returns() -> Result<()> {
    if std::env::var_os(LOCK_HELPER_ENV).is_some() {
        return Ok(());
    }
    let root = TempRoot::new()?;
    let writer = spawn_helper(root.path(), "publish")?;
    wait_for(writer, "publishing helper")?;

    let reader = spawn_helper(root.path(), "probe");
    wait_for(reader?, "probing helper")?;
    Ok(())
}

/// Verifies each publication merges the fragments already on disk.
///
/// This is the read-modify-write the lock exists for. Every child writes only
/// its own fragment, so an inventory that retained a second publisher's
/// entries can only have been produced by re-reading the fragment directory
/// after that peer's fragment landed. That is exactly the merge behaviour a
/// lock-less implementation would lose.
///
/// The children run in sequence rather than concurrently, and deliberately so.
/// The exclusion that makes concurrent publication safe is proved
/// deterministically by `artefact_lock_is_exclusive_across_processes`; racing
/// two children here would add a timing-dependent check whose failure also
/// depends on machine load, so a green run would prove less. Sequenced
/// spawning asserts only what this test is responsible for: that the merge
/// reads the live directory instead of caching a snapshot from before the
/// first write.
#[test]
fn each_publication_merges_the_fragments_already_published() -> Result<()> {
    if std::env::var_os(LOCK_HELPER_ENV).is_some() {
        return Ok(());
    }
    let root = TempRoot::new()?;
    for variant in VARIANT_CRATES {
        let writer = spawn_variant(root.path(), "publish-variant", Some(variant))?;
        wait_for(writer, &format!("{variant} publishing helper"))?;
    }

    let combined = fs::read_to_string(root.path().join("cli-identifiers.json"))
        .context("read the merged inventory published by both children")?;
    for variant in VARIANT_CRATES {
        ensure!(
            combined.contains(&format!("{variant}::Config")),
            "merged inventory must retain every publisher's entries, but the \
             {variant} entries are missing; a publication that read the \
             fragment directory before its peer wrote would drop them. \
             Inventory: {combined}",
        );
    }
    Ok(())
}

/// Re-enters the test binary as a lock helper in a separate process.
///
/// Normal test runs return immediately: the helper body only executes when
/// `spawn_helper` sets `LOCK_HELPER_ENV`. The helper lives inside the test
/// binary so it exercises the real `lock` implementation rather than a copy of
/// it.
#[test]
fn lock_helper_entry_point() -> Result<()> {
    if std::env::var_os(LOCK_HELPER_ENV).is_none() {
        return Ok(());
    }
    let root = std::env::var_os(LOCK_ROOT_ENV)
        .map(PathBuf::from)
        .context("helper requires the artefact lock root")?;
    let mode = std::env::var(LOCK_MODE_ENV).context("helper requires a lock mode")?;
    match mode.as_str() {
        "hold" => hold(&root),
        "contend" => contend(&root),
        "publish" => publish(&root),
        "publish-variant" => {
            let variant = std::env::var(LOCK_VARIANT_ENV)
                .context("variant publisher requires a variant name")?;
            publish_variant(&root, &variant)
        }
        "probe" => probe(&root),
        other => Err(anyhow::anyhow!("unknown helper mode {other}")),
    }
}

/// Acquires the lock, advertises ownership, then releases it.
fn hold(root: &Path) -> Result<()> {
    let guard = lock(root)?;
    fs::write(root.join(SENTINEL_FILE), SENTINEL_ACQUIRED).context("record lock ownership")?;
    std::thread::sleep(HOLD_PERIOD);
    // Write the release marker while the guard is still held, so every later
    // acquirer is guaranteed to observe it.
    fs::write(root.join(SENTINEL_FILE), SENTINEL_RELEASED).context("record lock release")?;
    drop(guard);
    Ok(())
}

/// Acquires the lock and fails when a holder was still advertising ownership.
fn contend(root: &Path) -> Result<()> {
    let _guard = lock(root)?;
    let observed = fs::read_to_string(root.join(SENTINEL_FILE))
        .context("contender requires the holder's sentinel")?;
    ensure!(
        observed == SENTINEL_RELEASED,
        "contender acquired the lock while the holder still reported it owned it; \
         observed {observed:?}, which shows no inter-process exclusion held",
    );
    Ok(())
}

/// Fails unless the lock is free, without blocking on a stuck holder.
fn probe(root: &Path) -> Result<()> {
    let free = is_free(root).context("probe the artefact lock")?;
    ensure!(
        free,
        "lock was still held after the publisher returned, so a later compiler \
         process would block indefinitely",
    );
    Ok(())
}

/// Runs a full fragment publication so the lock wraps real emission work.
///
/// This drives the production `publish` boundary rather than re-implementing
/// the sequence, so the exclusivity and release assertions observe exactly the
/// code the compiler runs. The entry set comes from a real identifier model,
/// built here through the same public builder the derive uses.
fn publish(root: &Path) -> Result<()> {
    publish_as(root, "fixture", "fixture")
}

/// Publishes one fragment under a crate-specific identity.
///
/// Each identity writes a distinct fragment filename and records a distinct
/// crate name, so the merged inventory shows whether a later publication
/// retained an earlier publication's entries.
fn publish_as(root: &Path, label: &str, crate_name: &str) -> Result<()> {
    let ident = Ident::new("Config", proc_macro2::Span::call_site());
    // The fragment filename mixes the source path and the line, so keying the
    // path by label keeps every identity's fragment distinct. The file has to
    // exist: `merge_fragments` treats a vanished source as a stale fragment
    // and drops its entries, which would silently empty this fixture.
    let source = Source {
        file: fixture_source_path(root, label)?,
        line: 1,
        column: 0,
    };
    let model = helper_model();
    publish::publish(
        &publish::Publication {
            root,
            ident: &ident,
            source: &source,
            crate_name,
            model: &model,
        },
        proc_macro2::Span::call_site(),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}

/// Creates a stand-in source file and returns its absolute path.
///
/// The path must be absolute because the merge probes it from whatever working
/// directory the test binary happens to run in.
fn fixture_source_path(root: &Path, label: &str) -> Result<String> {
    let path = root.join(format!("{label}-source.rs"));
    fs::write(&path, "// fixture source\n").context("create fixture source file")?;
    Ok(path.to_string_lossy().into_owned())
}

/// Publishes one named variant of the fixture source.
fn publish_variant(root: &Path, variant: &str) -> Result<()> {
    publish_as(root, variant, variant)
}

/// Builds a minimal identifier model for the helper's publication.
fn helper_model() -> LocalizationIds {
    LocalizationIds {
        base: LocalizationBase(String::from("fixture")),
        command: CommandIds {
            about_id: FluentMessageId(String::from("fixture-about")),
            long_about_id: FluentMessageId(String::from("fixture-long_about")),
            usage_id: FluentMessageId(String::from("fixture-usage")),
            version_id: FluentMessageId(String::from("fixture-version")),
            long_version_id: FluentMessageId(String::from("fixture-long_version")),
            after_help_id: FluentMessageId(String::from("fixture-after_help")),
            after_long_help_id: FluentMessageId(String::from("fixture-after_long_help")),
        },
        args: Vec::new(),
    }
}
