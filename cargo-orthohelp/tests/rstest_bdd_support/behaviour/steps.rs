//! Shared scenario state and re-exports for `cargo-orthohelp` behavioural steps.
//!
//! Defines [`OrthoHelpContext`], the rstest-bdd [`ScenarioState`] struct that
//! accumulates per-scenario data (workspace root, temporary output directory,
//! last command output, and cached IR path/content). Also exposes the
//! [`orthohelp_context`] rstest fixture that initialises this state, and
//! target-directory helpers ([`resolve_target_dir`], [`scenario_target_dir`])
//! shared by [`super::steps_cmd`] and [`super::steps_cache`].
//!
//! All other step modules import [`OrthoHelpContext`] and [`StepResult`] from
//! here; none of them import from each other directly.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, File, OpenOptions},
};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use serde_json::Value;
#[cfg(unix)]
use std::os::fd::OwnedFd;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;
#[cfg(windows)]
use windows::Win32::Foundation::{ERROR_LOCK_VIOLATION, HANDLE};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::{
    LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx,
};
#[cfg(windows)]
use windows::Win32::System::IO::OVERLAPPED;
#[cfg(windows)]
use windows::core::HRESULT;

use crate::fixtures;
use crate::process_runner;

pub use super::steps_cmd::run_orthohelp;

/// Error type for step definition failures.
pub type StepError = Box<dyn std::error::Error + Send + Sync>;

/// Result type for step definition operations.
pub type StepResult<T> = Result<T, StepError>;

const SCENARIO_TIMEOUT_SECS: u64 = 596;

/// Scenario state for cargo-orthohelp scenarios.
#[derive(Debug, ScenarioState)]
pub struct OrthoHelpContext {
    pub workspace_root: Slot<Utf8PathBuf>,
    pub target_dir: Slot<Utf8PathBuf>,
    pub deadline: Slot<Instant>,
    pub(super) cache_ownership: Slot<CacheOwnership>,
    pub out_dir: Slot<TempDir>,
    pub last_output: Slot<std::process::Output>,
    pub cache_ir_path: Slot<Utf8PathBuf>,
    pub cache_ir_content: Slot<String>,
    pub policy_package: Slot<String>,
}

/// Cross-process ownership of the shared BDD cache beneath Cargo's target.
#[derive(Debug)]
pub(super) struct CacheOwnership {
    _lock: CacheLock,
}

/// Holds an advisory lock for the lifetime of its capability-opened file.
#[cfg(unix)]
#[derive(Debug)]
struct CacheLock {
    _file: OwnedFd,
}

/// Holds a Windows byte-range lock until its capability-opened file closes.
#[cfg(windows)]
#[derive(Debug)]
struct CacheLock {
    _file: File,
}

impl CacheOwnership {
    fn acquire(target_dir: &Utf8Path, deadline: Instant) -> StepResult<Self> {
        let root = target_dir.join("orthohelp-bdd");
        Dir::create_ambient_dir_all(&root, ambient_authority())?;
        let directory = Dir::open_ambient_dir(&root, ambient_authority())?;
        let lock_file = directory.open_with(
            "cache.lock",
            OpenOptions::new().read(true).write(true).create(true),
        )?;
        Ok(Self {
            _lock: acquire_cache_lock(lock_file, deadline)?,
        })
    }
}

#[cfg(unix)]
fn acquire_cache_lock(lock_file: File, deadline: Instant) -> StepResult<CacheLock> {
    let owned_file = OwnedFd::from(lock_file);
    loop {
        match rustix::fs::flock(
            &owned_file,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        ) {
            Ok(()) => return Ok(CacheLock { _file: owned_file }),
            Err(lock_error) => {
                let io_error = std::io::Error::from_raw_os_error(lock_error.raw_os_error());
                if io_error.kind() != std::io::ErrorKind::WouldBlock {
                    return Err(io_error.into());
                }
                wait_for_cache_ownership_retry(deadline)?;
            }
        }
    }
}

#[cfg(windows)]
fn acquire_cache_lock(file: File, deadline: Instant) -> StepResult<CacheLock> {
    loop {
        let mut overlapped = OVERLAPPED::default();
        // SAFETY: `file` is a live handle opened through the capability
        // directory. The requested range spans this lock file, and
        // `LOCKFILE_FAIL_IMMEDIATELY` keeps the call synchronous.
        let result = unsafe {
            LockFileEx(
                HANDLE(file.as_raw_handle()),
                LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
                None,
                u32::MAX,
                u32::MAX,
                &mut overlapped,
            )
        };
        match result {
            Ok(()) => return Ok(CacheLock { _file: file }),
            Err(error) if error.code() == HRESULT::from_win32(ERROR_LOCK_VIOLATION.0) => {
                wait_for_cache_ownership_retry(deadline)?;
            }
            Err(error) => return Err(std::io::Error::other(error.to_string()).into()),
        }
    }
}

fn wait_for_cache_ownership_retry(deadline: Instant) -> StepResult<()> {
    if Instant::now() >= deadline {
        return Err("BDD scenario deadline elapsed waiting for shared cache ownership".into());
    }
    thread::sleep(Duration::from_millis(10));
    Ok(())
}

impl Default for OrthoHelpContext {
    fn default() -> Self {
        // The filesystem lease serializes scenarios across direct Cargo test
        // and separate nextest processes while they clear or reuse the shared
        // application cache. Cargo itself owns synchronization of build files.
        Self {
            workspace_root: Slot::new(),
            target_dir: Slot::new(),
            deadline: Slot::new(),
            cache_ownership: Slot::new(),
            out_dir: Slot::new(),
            last_output: Slot::new(),
            cache_ir_path: Slot::new(),
            cache_ir_content: Slot::new(),
            policy_package: Slot::new(),
        }
    }
}

/// Gets the output directory path from the context.
pub fn get_out_dir(ctx: &OrthoHelpContext) -> StepResult<Utf8PathBuf> {
    let out_dir = ctx
        .out_dir
        .with_ref(|dir| {
            Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
                .map_err(|p| format!("non-UTF-8 path: {}", p.display()))
        })
        .ok_or_else(|| "out_dir should be set".to_owned())??;
    Ok(out_dir)
}

/// Gets the workspace root path from the context.
pub fn get_workspace_root(ctx: &OrthoHelpContext) -> StepResult<Utf8PathBuf> {
    ctx.workspace_root
        .with_ref(Clone::clone)
        .ok_or_else(|| "workspace_root should be set".into())
}

pub(super) fn resolve_target_dir(
    workspace_root: &Utf8Path,
    deadline: Instant,
) -> StepResult<Utf8PathBuf> {
    let mut command = Command::new("cargo");
    crate::cargo_environment::CargoEnvironment::metadata().apply(&mut command);
    command
        .args(["metadata", "--offline", "--no-deps", "--format-version=1"])
        .current_dir(workspace_root.as_str());
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err("BDD scenario deadline elapsed before Cargo target discovery".into());
    }
    let output = process_runner::run_with_policy(
        &mut command,
        process_runner::Operation("discover the BDD Cargo target"),
        process_runner::Policy::discovery().bounded_by(remaining),
    )?;
    if !output.status.success() {
        return Err(format!(
            "Cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let target = metadata
        .get("target_directory")
        .and_then(Value::as_str)
        .ok_or("Cargo metadata omitted target_directory")?;
    Utf8PathBuf::from_path_buf(target.into())
        .map_err(|path| format!("Cargo target directory is not UTF-8: {}", path.display()).into())
}

pub(super) fn scenario_target_dir(ctx: &OrthoHelpContext) -> StepResult<Utf8PathBuf> {
    ctx.target_dir
        .with_ref(Clone::clone)
        .ok_or_else(|| "Cargo target directory should be set".into())
}

/// Provides a clean context for orthohelp scenarios.
#[fixture]
pub fn orthohelp_context() -> OrthoHelpContext {
    let workspace_root = match fixtures::workspace_root() {
        Ok(root) => root,
        // `rstest-bdd` requires this fixture to return the exact scenario
        // state type, so setup failures cannot be propagated as StepResult.
        Err(err) => panic!("workspace root should exist: {err}"),
    };
    let ctx = OrthoHelpContext::default();
    ctx.workspace_root.set(workspace_root.clone());
    let deadline = Instant::now() + Duration::from_secs(SCENARIO_TIMEOUT_SECS);
    ctx.deadline.set(deadline);
    let target_dir = match resolve_target_dir(&workspace_root, deadline) {
        Ok(target) => target,
        Err(err) => panic!("Cargo target should be discoverable: {err}"),
    };
    let ownership = match CacheOwnership::acquire(&target_dir, deadline) {
        Ok(ownership) => ownership,
        Err(err) => panic!("shared BDD cache should be owned: {err}"),
    };
    ctx.target_dir.set(target_dir);
    ctx.cache_ownership.set(ownership);
    let out_dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        // `rstest-bdd` requires this fixture to return the exact scenario
        // state type, so setup failures cannot be propagated as StepResult.
        Err(err) => panic!("temporary output directory should be created: {err}"),
    };
    ctx.out_dir.set(out_dir);
    ctx
}
