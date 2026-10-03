//! Shared helpers for BDD step definitions.
//!
//! Several step modules repeat the same "guard, validate, and populate a
//! scalar slot" shape, and the same "take the slot's value or fail with a
//! descriptive error" shape. They also repeat the same "write one file into an
//! isolated temporary directory and share one environment across both injected
//! source roles" shape. Centralising all three here keeps individual step
//! modules focused on scenario wiring rather than boilerplate.

use anyhow::{Context as _, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, SharedEnvSource, SharedScanEnvSource};
use rstest_bdd::Slot;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Populates `slot` with `value`, failing when the slot already holds a
/// value.
///
/// Use [`set_nonblank_scalar_once`] instead when blank (whitespace-only)
/// input must also be rejected.
///
/// # Examples
/// ```
/// let slot = Slot::<String>::default();
/// assert!(set_scalar_once(&slot, "first", "value").is_ok());
/// assert!(set_scalar_once(&slot, "second", "value").is_err());
/// ```
pub(crate) fn set_scalar_once(
    slot: &Slot<String>,
    value: impl Into<String>,
    label: &str,
) -> Result<()> {
    ensure!(slot.is_empty(), "{label} already initialised");
    slot.set(value.into());
    Ok(())
}

/// As [`set_scalar_once`], but first rejects blank (whitespace-only) input.
///
/// # Examples
/// ```
/// let slot = Slot::<String>::default();
/// assert!(set_nonblank_scalar_once(&slot, "   ", "value").is_err());
/// assert!(set_nonblank_scalar_once(&slot, "value", "value").is_ok());
/// ```
pub(crate) fn set_nonblank_scalar_once(
    slot: &Slot<String>,
    value: impl Into<String>,
    label: &str,
) -> Result<()> {
    let value = value.into();
    ensure!(!value.trim().is_empty(), "{label} must not be empty");
    set_scalar_once(slot, value, label)
}

/// Extension trait collapsing the repeated
/// `slot.take().ok_or_else(|| anyhow!(...))` boilerplate found throughout
/// the step definitions.
pub(crate) trait SlotTakeOrExt<T> {
    /// Takes the slot's value, or fails with `message` when the slot is
    /// empty.
    ///
    /// # Examples
    /// ```
    /// let populated = Slot::<String>::default();
    /// populated.set("value".to_string());
    /// assert_eq!(populated.take_or("missing value").expect("value"), "value");
    ///
    /// let empty = Slot::<String>::default();
    /// assert!(empty.take_or("missing value").is_err());
    /// ```
    fn take_or(&self, message: &str) -> Result<T>;
}

impl<T> SlotTakeOrExt<T> for Slot<T> {
    fn take_or(&self, message: &str) -> Result<T> {
        self.take().ok_or_else(|| anyhow!("{message}"))
    }
}

/// Writes `contents` to `file_name` inside a fresh temporary directory.
///
/// Returns the directory guard alongside the absolute path of the file. Pass
/// `None` for `contents` to derive the path without writing anything, which
/// preserves scenarios that deliberately leave a selected file absent.
///
/// The caller **must** keep the returned guard alive until the loader has read
/// the fixture. Dropping it deletes the directory, and the loader would then
/// see a missing file instead of the scenario's contents.
///
/// # Examples
///
/// ```rust
/// let (guard, path) =
///     config_fixture(".ddlint.toml", Some("rules = []")).expect("fixture");
/// assert!(path.ends_with(".ddlint.toml"));
/// drop(guard);
/// ```
pub(crate) fn config_fixture(
    file_name: impl AsRef<Path>,
    contents: Option<&str>,
) -> Result<(tempfile::TempDir, PathBuf)> {
    let file_name = file_name.as_ref();
    let fixture_dir = tempfile::tempdir().context("create configuration fixture directory")?;
    let file_path = fixture_dir.path().join(file_name);
    if let Some(contents) = contents {
        let fixture = Dir::open_ambient_dir(fixture_dir.path(), ambient_authority())
            .context("open configuration fixture directory")?;
        fixture
            .write(file_name, contents.as_bytes())
            .with_context(|| format!("write configuration fixture {}", file_path.display()))?;
    }
    Ok((fixture_dir, file_path))
}

/// Splits one environment into the discovery and merge source roles.
///
/// Both returned handles share the single owned [`MapEnv`], matching loader
/// APIs that take a discovery source and a merge source separately while both
/// read the same environment.
///
/// # Examples
///
/// ```rust
/// let (discovery, merge) = shared_sources(MapEnv::new().with_var("RULES", "cli"));
/// assert_eq!(discovery.get("RULES").as_deref(), Some("cli".as_ref()));
/// assert_eq!(merge.get("RULES").as_deref(), Some("cli".as_ref()));
/// ```
pub(crate) fn shared_sources(environment: MapEnv) -> (SharedEnvSource, SharedScanEnvSource) {
    let source = Arc::new(environment);
    (source.clone(), source)
}

/// Builds an environment that pins `HOME` and the XDG configuration roots
/// inside `root`.
///
/// Subcommand discovery falls back to the user's home and XDG directories when
/// no explicit selector is present. Pointing those variables at a fixture
/// directory keeps such a scenario independent of the machine running the
/// suite.
///
/// # Examples
///
/// ```rust
/// let environment = isolated_home_env(Path::new("/tmp/fixture"));
/// assert_eq!(
///     environment.get("XDG_CONFIG_HOME").as_deref(),
///     Some("/tmp/fixture/xdg-home".as_ref())
/// );
/// ```
// Only the `serde_json`-gated subcommand step modules isolate the home
// directories, so this helper would be dead code in a build without that
// feature.
#[cfg(feature = "serde_json")]
pub(crate) fn isolated_home_env(root: &Path) -> MapEnv {
    MapEnv::new()
        .with_var("HOME", root.join("home"))
        .with_var("XDG_CONFIG_HOME", root.join("xdg-home"))
        .with_var("XDG_CONFIG_DIRS", root.join("xdg-dirs"))
}
