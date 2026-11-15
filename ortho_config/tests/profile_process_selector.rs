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

#[path = "support/process_child.rs"]
mod process_child;

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
    let empty_root = tempfile::tempdir().context("create the empty configuration tree")?;
    let config_path = write_selector_fixture(fixture_dir.path())?;

    // Both cases read the *process* environment by design, so each runs in a
    // child started with exactly the variables it needs; this process's own
    // environment is never changed.
    let run = |case: &str, selector: std::ffi::OsString| {
        process_child::run(
            CHILD_TEST,
            case,
            &process_child::ChildEnvironment {
                dir: fixture_dir.path(),
                empty_root: empty_root.path(),
                vars: &[
                    (
                        "PROCESS_SELECTOR_CONFIG_PATH",
                        config_path.clone().into_os_string(),
                    ),
                    ("PROCESS_SELECTOR_PROFILE", selector),
                ],
            },
        )
    };
    // The control pins the fixture to profile `ci`, so the case cannot be
    // blamed on an unreachable fixture or an inoperative selector variable.
    run("control", "ci".into())?;
    run("non_unicode", non_unicode_selector())
}

/// Name of the child test in this binary.
const CHILD_TEST: &str = "process_selector_child";

/// Child half of [`a_non_unicode_process_selector_fails_the_load`].
///
/// A no-op unless a parent started it with a case.
#[test]
fn process_selector_child() -> Result<()> {
    match process_child::case().as_deref() {
        Some("control") => assert_control_reaches_profile_ci(),
        Some("non_unicode") => assert_non_unicode_selector_fails(),
        _ => Ok(()),
    }
}

/// A valid selector must reach profile `ci` and yield 7. The struct default (1)
/// and the file base (3) are both distinguishable from it.
fn assert_control_reaches_profile_ci() -> Result<()> {
    let control = ProcessSelectorConfig::load_with_profile_from_iter(["process-selector"])
        .map_err(|error| anyhow::anyhow!(error))?;
    let control_retries = control.config().retries;
    ensure!(
        control_retries == 7,
        "the fixture must be discoverable and profile `ci` must apply: got {control_retries}"
    );
    Ok(())
}

/// The same run with a selector that cannot be Unicode must fail the load.
fn assert_non_unicode_selector_fails() -> Result<()> {
    let control_retries = 7;
    let case = ProcessSelectorConfig::load_with_profile_from_iter(["process-selector"]);

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
