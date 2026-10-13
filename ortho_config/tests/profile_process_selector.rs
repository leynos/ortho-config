//! Process-backed profile selector behaviour for non-Unicode values.
//!
//! The process-backed selection path reads its selector through the process
//! environment, while the injected-source path reads the caller's `EnvSource`.
//! Both must report a selector that cannot be Unicode rather than falling back
//! to the unprofiled configuration: a caller who set a selector and then
//! received base values would have no signal that the profile was skipped.
//!
//! These cases exercise the process path, which reaches the selector through
//! clap's generated `env = ...` binding rather than a handwritten read. They
//! deliberately avoid `load_with_profile()`: that entry point consumes
//! `std::env::args_os()`, so a test harness's own flags would reach clap and the
//! load would fail for an unrelated parse error. Passing the program name
//! explicitly keeps the arguments inert and leaves the environment as the only
//! input under test.
//!
//! The non-Unicode case needs a raw-byte `OsString`, which has no portable
//! constructor, so the file is gated to Unix as a whole rather than leaving its
//! fixtures and imports unused on other platforms.

#![cfg(unix)]

use anyhow::{Context as _, Result, ensure};
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};
use test_helpers::env as test_env;

/// Opts into profile support so the process selector lookup path is exercised.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "PROCESS_SELECTOR_", profiles)]
struct ProcessSelectorConfig {
    #[ortho_config(default = 1)]
    retries: u8,
}

/// Write a config file defining profile `ci`, returning its path.
///
/// The write goes through a `cap_std::fs::Dir` handle rather than `std::fs`,
/// as the repository's lint suite requires.
fn write_selector_fixture(dir: &std::path::Path) -> Result<std::path::PathBuf> {
    use cap_std::{ambient_authority, fs::Dir};

    let cap = Dir::open_ambient_dir(dir, ambient_authority())
        .context("open process selector fixture directory")?;
    cap.write(
        "process_selector.toml",
        b"retries = 3\n[profile.ci]\nretries = 7\n",
    )
    .context("write process selector fixture")?;
    Ok(dir.join("process_selector.toml"))
}

/// Build the non-UTF-8 selector used by the case step.
///
/// The fixture defines profile `ci`, so a loader that dropped the selector
/// would still succeed with the base value of `3`. Appending `0xFF` makes the
/// value non-representable however the preceding bytes happen to decode.
///
/// Gated to Unix because building a non-UTF-8 `OsString` from raw bytes needs
/// `std::os::unix::ffi::OsStringExt`; there is no portable constructor.
fn non_unicode_selector() -> std::ffi::OsString {
    use std::os::unix::ffi::OsStringExt as _;

    let mut raw = b"ci".to_vec();
    raw.push(0xFF);
    std::ffi::OsString::from_vec(raw)
}

/// A non-Unicode process selector must fail the load, not fall back to base.
///
/// The control step pins the fixture to profile `ci` through the process
/// environment, so the case step cannot be blamed on an unreachable fixture or
/// an inoperative selector variable.
///
/// The non-Unicode case is Unix-only, so the whole test is gated to Unix rather
/// than leaving the case to be skipped at runtime.
#[test]
fn a_non_unicode_process_selector_fails_the_load() -> Result<()> {
    let fixture_dir = tempfile::tempdir().context("create process selector fixture")?;
    let config_path = write_selector_fixture(fixture_dir.path())?;

    let lock = test_env::lock();
    let _path_guard = lock.set_var("PROCESS_SELECTOR_CONFIG_PATH", &config_path);

    // CONTROL: a valid selector must reach profile `ci` and yield 7. The struct
    // default (1) and the file base (3) are both distinguishable from it.
    let control_guard = lock.set_var("PROCESS_SELECTOR_PROFILE", "ci");
    let control = ProcessSelectorConfig::load_with_profile_from_iter(["process-selector"])
        .map_err(|error| anyhow::anyhow!(error))?;
    let control_retries = control.config().retries;
    ensure!(
        control_retries == 7,
        "the fixture must be discoverable and profile `ci` must apply: got {control_retries}"
    );
    drop(control_guard);

    // CASE: the same run with a selector that cannot be Unicode. A success here
    // would be returned in place of the `expect_err` panic, so the case is
    // checked rather than merely awaited.
    let case_guard = lock.set_var("PROCESS_SELECTOR_PROFILE", non_unicode_selector());
    let case = ProcessSelectorConfig::load_with_profile_from_iter(["process-selector"]);
    drop(case_guard);

    let case_retries = match case {
        Err(error) => {
            let message = error.to_string();
            // The diagnostic must name the encoding fault; a bare parse failure
            // would leave a caller no way to find the offending value.
            ensure!(
                message.contains("invalid UTF-8"),
                "control reached profile `ci` with retries={control_retries}, but the \
                 non-Unicode case failed for an unrelated reason: {message}"
            );
            return Ok(());
        }
        Ok(outcome) => outcome.config().retries,
    };

    anyhow::bail!(
        "a non-Unicode process selector must fail the load, not succeed unprofiled: \
         control reached profile `ci` with retries={control_retries}, but the non-Unicode \
         selector loaded retries={case_retries}"
    )
}
