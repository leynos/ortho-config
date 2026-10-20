//! Cargo helpers owned by the identifier-artefact integration test.
//!
//! The fixture uses a private target nested under Cargo's configured target
//! directory so its clean operations cannot delete workspace test binaries.
//! Cargo still uses the default shared package cache.

use super::{cargo_environment, process_runner, scenario};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use serde_json::Value;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

const MAX_CARGO_RECORD_BYTES: usize = 128 * 1024;
const CACHEDIR_TAG: &str = "Signature: 8a477f597d28d172789f06886806bc55\n# This file is a cache directory tag created by cargo.\n# For information about cache directory tags see https://bford.info/cachedir/\n";

#[derive(Default)]
struct CargoMessages {
    package_id: String,
    line: Vec<u8>,
    out_dir: Option<PathBuf>,
    error: Option<String>,
}

impl CargoMessages {
    fn observe(&mut self, bytes: &[u8]) {
        for byte in bytes {
            if !self.observe_byte(*byte) {
                break;
            }
        }
    }

    fn observe_byte(&mut self, byte: u8) -> bool {
        if byte == b'\n' {
            return self.finish_line();
        }
        if self.line.len() == MAX_CARGO_RECORD_BYTES {
            self.error = Some("Cargo JSON record exceeded 128 KiB".to_owned());
            return false;
        }
        self.line.push(byte);
        true
    }

    fn finish_line(&mut self) -> bool {
        match self.parse_line() {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }

    fn finish(&mut self) -> Result<Option<PathBuf>> {
        if !self.line.is_empty() {
            self.parse_line()?;
        }
        if let Some(error) = self.error.take() {
            anyhow::bail!(error);
        }
        Ok(self.out_dir.take())
    }

    fn parse_line(&mut self) -> Result<()> {
        if self.line.is_empty() {
            return Ok(());
        }
        let record: Value =
            serde_json::from_slice(&self.line).context("parse Cargo JSON record")?;
        self.line.clear();
        if record.get("reason").and_then(Value::as_str) == Some("build-script-executed")
            && record.get("package_id").and_then(Value::as_str) == Some(self.package_id.as_str())
        {
            ensure!(
                self.out_dir.is_none(),
                "Cargo reported more than one fixture build-script directory"
            );
            self.out_dir = Some(PathBuf::from(
                record
                    .get("out_dir")
                    .and_then(Value::as_str)
                    .context("fixture build-script message omitted out_dir")?,
            ));
        }
        Ok(())
    }
}

fn lock_cargo_messages(messages: &Mutex<CargoMessages>) -> MutexGuard<'_, CargoMessages> {
    messages
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Owns the fixture's private Cargo target for this test run.
pub(super) struct FixtureCargo {
    repository: PathBuf,
    target_directory: PathBuf,
    package_id: String,
    deadline: Instant,
    _target_owner: tempfile::TempDir,
}

impl FixtureCargo {
    fn bounded_policy(
        &self,
        policy: process_runner::Policy,
        operation: &str,
    ) -> Result<process_runner::Policy> {
        let remaining = scenario::remaining(self.deadline)
            .with_context(|| format!("{operation}: scenario deadline elapsed"))?;
        Ok(policy.bounded_by(remaining))
    }
}

/// Discover the fixture package and allocate a target private to this test.
pub(super) fn fixture_cargo() -> Result<FixtureCargo> {
    let deadline = scenario::deadline();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("find fixture workspace")?
        .to_path_buf();
    let mut command = Command::new("cargo");
    cargo_environment::CargoEnvironment::metadata().apply(&mut command);
    command
        .args(["metadata", "--offline", "--no-deps", "--format-version=1"])
        .current_dir(&repository);
    let remaining =
        scenario::remaining(deadline).context("discover Cargo target for identifier artefact")?;
    let output = process_runner::run_with_policy(
        &mut command,
        process_runner::Operation("discover Cargo target for identifier artefact"),
        process_runner::Policy::discovery().bounded_by(remaining),
    )?;
    ensure!(
        output.status.success(),
        "Cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value =
        serde_json::from_slice(&output.stdout).context("parse Cargo fixture metadata")?;
    let workspace_target = metadata
        .get("target_directory")
        .and_then(Value::as_str)
        .context("Cargo metadata omitted its target directory")?;
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .context("Cargo metadata omitted its workspace packages")?;
    let package_id = packages
        .iter()
        .find(|package| package.get("name").and_then(Value::as_str) == Some("orthohelp_fixture"))
        .and_then(|package| package.get("id"))
        .and_then(Value::as_str)
        .context("Cargo metadata omitted orthohelp_fixture's package id")?;
    let target_owner = tempfile::Builder::new()
        .prefix("identifier-artefact-target-")
        .tempdir_in(workspace_target)
        .context("create private Cargo target")?;
    let target_directory = target_owner.path().to_path_buf();
    Dir::open_ambient_dir(&target_directory, ambient_authority())?
        .write("CACHEDIR.TAG", CACHEDIR_TAG.as_bytes())
        .context("write Cargo cache-directory tag")?;
    Ok(FixtureCargo {
        repository,
        target_directory,
        package_id: package_id.to_owned(),
        deadline,
        _target_owner: target_owner,
    })
}

/// Build the fixture with optional artefact emission and find its reported output directory.
pub(super) fn build_fixture(
    emit: bool,
    cargo: &FixtureCargo,
    previous_out_dir: Option<&Path>,
) -> Result<PathBuf> {
    let mut command = Command::new("cargo");
    cargo_environment::CargoEnvironment::build().apply(&mut command);
    command
        .args([
            "build",
            "-p",
            "orthohelp_fixture",
            "--message-format=json-render-diagnostics",
        ])
        .arg("--target-dir")
        .arg(&cargo.target_directory)
        .current_dir(&cargo.repository)
        .env_remove("ORTHO_CONFIG_EMIT_IDENTIFIERS");
    if emit {
        command.env("ORTHO_CONFIG_EMIT_IDENTIFIERS", "1");
    }
    let messages = Arc::new(Mutex::new(CargoMessages {
        package_id: cargo.package_id.clone(),
        ..CargoMessages::default()
    }));
    let observed_messages = Arc::clone(&messages);
    let output = process_runner::run_with_observer(
        &mut command,
        process_runner::Operation("build identifier artefact fixture"),
        cargo.bounded_policy(
            process_runner::Policy::build(),
            "build identifier artefact fixture",
        )?,
        move |chunk| lock_cargo_messages(&observed_messages).observe(chunk),
    )
    .map_err(|error| retain_runner_error(error, "build identifier artefact fixture"))?;
    ensure!(
        output.status.success(),
        "fixture build should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    lock_cargo_messages(&messages)
        .finish()?
        .or_else(|| previous_out_dir.map(Path::to_path_buf))
        .context("Cargo output omitted the fixture build-script directory")
}

/// Remove only the fixture's build output in its private target directory.
pub(super) fn clean_fixture(cargo: &FixtureCargo) -> Result<()> {
    let mut command = Command::new("cargo");
    cargo_environment::CargoEnvironment::build().apply(&mut command);
    let output = process_runner::run_with_policy(
        command
            .args(["clean", "-p", "orthohelp_fixture", "--target-dir"])
            .arg(&cargo.target_directory)
            .current_dir(&cargo.repository),
        process_runner::Operation("clean identifier artefact fixture outputs"),
        cargo.bounded_policy(
            process_runner::Policy::build(),
            "clean identifier artefact fixture outputs",
        )?,
    )
    .map_err(|error| retain_runner_error(error, "clean identifier artefact fixture outputs"))?;
    ensure!(
        output.status.success(),
        "fixture package clean should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// Return the artefact path, if the fixture build emitted one.
pub(super) fn artefact_path(out_dir: &Path) -> Result<Option<PathBuf>> {
    let directory = match Dir::open_ambient_dir(out_dir, ambient_authority()) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).context("open fixture build-script output directory");
        }
    };
    let relative_path = Path::new("ortho-config/cli-identifiers.json");
    Ok(directory
        .exists(relative_path)
        .then(|| out_dir.join(relative_path)))
}

/// Read the emitted identifier artefact, failing when it is absent.
pub(super) fn artefact(out_dir: &Path) -> Result<String> {
    let directory = Dir::open_ambient_dir(out_dir, ambient_authority())
        .context("open fixture build-script output directory")?;
    directory
        .read_to_string("ortho-config/cli-identifiers.json")
        .context("read identifier artefact")
}

fn retain_runner_error(error: anyhow::Error, operation: &str) -> anyhow::Error {
    if process_runner::deadline_exceeded(&error) {
        error.context(format!("{operation} exceeded its overall deadline"))
    } else if process_runner::cleanup_incomplete(&error) {
        error.context(format!("{operation} cleanup was not confirmed"))
    } else {
        error.context(format!("{operation} subprocess failed"))
    }
}
