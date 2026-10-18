//! Shared documentation-build discovery and cross-process ownership policy.
//!
//! This test-only state is shared by the executable Rust examples and the CLI
//! documentation integration target. Compilation and artifact extraction stay
//! in `build_cache`; callers holding this state must not recursively acquire it.

use anyhow::{Context, Result, ensure};
use cap_std::{
    ambient_authority,
    fs::{Dir, File, OpenOptions},
};
use fs4::{FileExt, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::cargo_runner;
use crate::process_runner::{self, Operation, Policy};

/// Sidecar ownership state for builds reusing Cargo's resolved target.
pub(super) struct BuildState {
    pub(super) root: PathBuf,
    pub(super) target: PathBuf,
}

impl BuildState {
    /// Discover Cargo's configured target and separate ownership state.
    ///
    /// A target `/repo/target` remains the compiler cache. Its sidecar lock
    /// lives under `/repo/target/documentation-examples/v2`. Cargo releases its
    /// build lock before executing tests; a regression checks that contract.
    pub(super) fn discover(repository: &Path, remaining: Duration) -> Result<Self> {
        let mut command = cargo_runner::prepare_metadata_command(repository);
        command.args(["metadata", "--offline", "--no-deps", "--format-version=1"]);
        let output = process_runner::run_with_policy(
            &mut command,
            Operation("discover documentation build cache"),
            Policy::discovery().with_timeout(remaining.min(Policy::discovery().timeout())),
        )?;
        ensure!(
            output.status.success(),
            "Cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
            .context("read Cargo metadata target directory")?;
        let target = metadata
            .get("target_directory")
            .and_then(serde_json::Value::as_str)
            .context("Cargo metadata omitted target_directory")?;
        Ok(Self::for_parent_target(Path::new(target)))
    }

    /// Select cache state from an injected Cargo-resolved target, without env mutation.
    pub(super) fn for_parent_target(target: &Path) -> Self {
        Self {
            root: target.join("documentation-examples/v2"),
            target: target.to_path_buf(),
        }
    }

    /// Hold the same ownership lock while a documented CLI builds its bridge.
    ///
    /// The callback receives sidecar state and the Cargo-resolved target. It
    /// must not recursively acquire
    /// this lock, and must supervise its child with the supplied remaining time.
    pub(super) fn with_owned_state<T>(
        repository: &Path,
        deadline: Instant,
        run: impl FnOnce(&Path, &Path, Duration) -> Result<T>,
    ) -> Result<T> {
        let initial_remaining = deadline
            .checked_duration_since(Instant::now())
            .context("documentation scenario deadline elapsed before cache discovery")?;
        let cache = Self::discover(repository, initial_remaining)?;
        cache.run_owned_state(deadline, run)
    }

    // This private entry point lets policy tests inject a cache without Cargo
    // discovery. The callback is opaque: nested compiler pipes may be hidden.
    pub(super) fn run_owned_state<T>(
        &self,
        deadline: Instant,
        run: impl FnOnce(&Path, &Path, Duration) -> Result<T>,
    ) -> Result<T> {
        let _ownership = self.acquire(deadline)?;
        let owned_remaining = deadline
            .checked_duration_since(Instant::now())
            .context("documentation scenario deadline elapsed waiting for cache ownership")?;
        let directory = Dir::open_ambient_dir(&self.root, ambient_authority())?;
        ensure!(
            !directory.exists("build.in-progress"),
            "documentation cache needs owner recovery: {}",
            self.root.display()
        );
        directory.write(
            "build.in-progress",
            "documented CLI owns nested build paths\n",
        )?;
        match run(&self.root, &self.target, owned_remaining) {
            Ok(value) => finish_owned(&directory, Ok(value)),
            Err(error) => Err(error.context(
                "opaque nested build failed; cache recovery requires confirmation that its compiler writers stopped",
            )),
        }
    }

    pub(super) fn acquire(&self, deadline: Instant) -> Result<File> {
        Dir::create_ambient_dir_all(&self.root, ambient_authority())?;
        let directory = Dir::open_ambient_dir(&self.root, ambient_authority())?;
        let lock = directory
            .open_with(
                "build.lock",
                OpenOptions::new().read(true).write(true).create(true),
            )?
            .into_std();
        loop {
            match FileExt::try_lock(&lock) {
                Ok(()) => return Ok(File::from_std(lock)),
                Err(TryLockError::Error(error)) => {
                    return Err(error).context("lock documentation build cache");
                }
                Err(TryLockError::WouldBlock) => {
                    ensure!(
                        Instant::now() < deadline,
                        "documentation build deadline elapsed waiting for build-and-snapshot lock: cache={}",
                        self.root.display()
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
}

// Only abrupt termination or incomplete cleanup leaves the recovery marker.
// Normal compiler errors, snapshot errors, and preparation failures must not
// poison a cache whose owned subprocesses have finished.
pub(super) fn finish_owned<T>(directory: &Dir, result: Result<T>) -> Result<T> {
    if result
        .as_ref()
        .err()
        .is_some_and(process_runner::cleanup_incomplete)
    {
        return result;
    }
    match (result, directory.remove_file("build.in-progress")) {
        (outcome, Ok(())) => outcome,
        (Err(primary), Err(cleanup)) => {
            Err(primary.context(format!("remove cache ownership marker: {cleanup}")))
        }
        (Ok(_), Err(cleanup)) => Err(cleanup).context("remove cache ownership marker"),
    }
}
