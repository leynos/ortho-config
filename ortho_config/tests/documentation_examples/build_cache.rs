//! Cross-process build-and-snapshot ownership for documentation tests only.
//!
//! Cargo owns compiler fingerprints. This helper owns a separate advisory lock
//! spanning the build and executable copy; Cargo's own build lock ends too soon
//! to protect a caller's later copy from another same-named binary build.

#[path = "build_artifacts.rs"]
mod build_artifacts;

use anyhow::{Context, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(super) use super::build_state::BuildState as BuildCache;
use super::build_state::finish_owned;
use super::cargo_runner;
use crate::process_runner::{self, Operation, Policy};
use build_artifacts::Artifacts;

/// Caller-owned inputs for one documentation build and private snapshot.
#[derive(Clone, Copy)]
pub(super) struct BuildInputs<'a> {
    pub(super) package: &'a Path,
    pub(super) destination: &'a Path,
    pub(super) arguments: &'a [&'a str],
}

#[derive(Clone, Copy)]
struct BuildRequest<'a> {
    package: &'a Path,
    destination: &'a Path,
    started: Instant,
    timeout: Duration,
}

impl BuildCache {
    /// Build and copy executables before releasing ownership of shared paths.
    ///
    /// `arguments` selects Cargo build targets; `destination` must be private to
    /// the caller. The single deadline includes cache discovery and lock waits.
    pub(super) fn build_before(
        repository: &Path,
        inputs: BuildInputs<'_>,
        deadline: Instant,
    ) -> Result<BTreeMap<String, PathBuf>> {
        let started = Instant::now();
        let timeout = deadline
            .checked_duration_since(started)
            .context("documentation scenario exhausted before build")?;
        let cache = Self::discover(repository, timeout)?;
        cache.build_in(inputs, started, timeout)
    }

    fn build_in(
        &self,
        inputs: BuildInputs<'_>,
        started: Instant,
        timeout: Duration,
    ) -> Result<BTreeMap<String, PathBuf>> {
        self.build_with(
            BuildRequest {
                package: inputs.package,
                destination: inputs.destination,
                started,
                timeout,
            },
            |_cache, deadline| {
                // Linker-discovery scripts are invocation inputs, so keep them
                // in this private workspace rather than shared ownership state.
                let mut command = cargo_runner::prepare_cargo_command_for_target_before(
                    inputs.package,
                    inputs.package,
                    &self.target,
                    deadline,
                )?;
                command
                    .args([
                        "build",
                        "--offline",
                        "--message-format=json-render-diagnostics",
                    ])
                    .args(inputs.arguments);
                Ok(command)
            },
        )
    }

    // The command factory is private to this helper. Regression tests inject a
    // controlled compiler protocol peer without invoking a live toolchain.
    fn build_with(
        &self,
        request: BuildRequest<'_>,
        prepare: impl FnOnce(&Path, Instant) -> Result<Command>,
    ) -> Result<BTreeMap<String, PathBuf>> {
        let deadline = request.started + request.timeout;
        let _ownership = self.acquire(deadline)?;
        let directory = Dir::open_ambient_dir(&self.root, ambient_authority())?;
        // An interrupted build may leave a compiler owned by an external
        // admission service. Do not reuse its paths without owner recovery.
        ensure!(
            !directory.exists("build.in-progress"),
            "documentation cache needs owner recovery after an interrupted build: {}",
            self.root.display()
        );
        directory.write(
            "build.in-progress",
            format!(
                "workspace={} pid={}\n",
                request.package.display(),
                std::process::id()
            ),
        )?;
        let mut is_build_lifetime_unconfirmed = false;
        let result = self.run_and_snapshot(request, prepare, &mut is_build_lifetime_unconfirmed);
        finish_observed_build(&directory, result, is_build_lifetime_unconfirmed)
    }

    fn run_and_snapshot(
        &self,
        request: BuildRequest<'_>,
        prepare: impl FnOnce(&Path, Instant) -> Result<Command>,
        is_build_lifetime_unconfirmed: &mut bool,
    ) -> Result<BTreeMap<String, PathBuf>> {
        let deadline = request.started + request.timeout;
        let mut command = prepare(&self.root, deadline)?;
        let artifacts = Arc::new(Mutex::new(Artifacts::default()));
        let observed = Arc::clone(&artifacts);
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .context("documentation build deadline elapsed during preparation")?;
        *is_build_lifetime_unconfirmed = true;
        let output = process_runner::run_with_observer(
            &mut command,
            Operation("build documented Rust and snapshot executables"),
            Policy::new(
                "build",
                remaining.min(Policy::build().timeout()),
                Duration::from_secs(2),
                1024 * 1024,
            ),
            move |bytes| observe_artifacts(&observed, bytes),
        )
        .with_context(|| {
            format!(
                "workspace={} cache={} lock_wait_included=true elapsed={:?} deadline={:?}",
                request.package.display(),
                self.root.display(),
                request.started.elapsed(),
                request.timeout,
            )
        })?;
        *is_build_lifetime_unconfirmed = false;
        ensure!(
            output.status.success(),
            "documented Rust failed to compile:\nstdout (Cargo JSON):\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        ensure!(
            Instant::now() < deadline,
            "documentation build deadline elapsed before snapshot"
        );
        artifacts
            .lock()
            .map_err(|_| anyhow!("Cargo artefact observer was poisoned"))?
            .snapshot(request.destination)
    }
}

fn finish_observed_build<T>(
    directory: &Dir,
    result: Result<T>,
    lifetime_unconfirmed: bool,
) -> Result<T> {
    match result {
        Err(error) if lifetime_unconfirmed => Err(error.context(
            "Cargo supervision failed; external compiler writer lifetime is unconfirmed; cache recovery requires owner verification",
        )),
        outcome => finish_owned(directory, outcome),
    }
}

fn observe_artifacts(artifacts: &Mutex<Artifacts>, bytes: &[u8]) {
    if let Ok(mut state) = artifacts.lock() {
        state.consume(bytes);
    }
}

#[cfg(test)]
use super::scenario;

#[cfg(test)]
#[path = "build_cache_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "build_cache_process_tests.rs"]
mod process_tests;
