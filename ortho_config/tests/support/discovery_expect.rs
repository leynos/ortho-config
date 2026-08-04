//! Expected discovery candidates, derived from the active feature set.
//!
//! Asserting on exact candidate *sequences* rather than "does the list contain
//! something under this prefix" is the point of these helpers. A containment
//! check passes whether a candidate is first or last, so it cannot detect the
//! failure that actually matters: a base directory overtaking the selector, or
//! a project root overtaking the user's home. Precedence *is* the contract.
//!
//! The expectations must therefore be built rather than written out, because
//! `json5` and `yaml` each add variant filenames to every base-directory group.
//! A handwritten list would be right under `--no-default-features` and wrong
//! under `--all-features`, so it would be corrected into a containment check
//! the first time CI disagreed with a developer's machine.

use std::path::{Path, PathBuf};

/// The application name used across the injected-discovery suites.
pub const APP: &str = "demo";
/// The selector variable name used across the injected-discovery suites.
pub const SELECTOR: &str = "DEMO_CONFIG";
/// The single deterministic project root used across the suites.
pub const PROJECT_ROOT: &str = "/workspace";
/// The dotfile `ConfigDiscoveryBuilder` derives for [`APP`].
pub const DOTFILE: &str = ".demo.toml";

/// Candidates generated for one base directory, in the order discovery emits.
///
/// Mirrors `ConfigDiscovery::candidates_for_base`: the nested canonical file
/// first, then the dotfile beside it, then one entry per enabled format
/// variant. The `cfg` arms here must track that function; a mismatch shows up
/// immediately as a sequence assertion failure rather than as a silent gap.
#[must_use]
pub fn base_group(directory: impl AsRef<Path>) -> Vec<PathBuf> {
    let base = directory.as_ref();
    let nested = base.join(APP);

    let mut group = vec![nested.join("config.toml"), base.join(DOTFILE)];

    #[cfg(feature = "json5")]
    {
        group.push(nested.join("config.json"));
        group.push(nested.join("config.json5"));
    }
    #[cfg(feature = "yaml")]
    {
        group.push(nested.join("config.yaml"));
        group.push(nested.join("config.yml"));
    }

    group
}

/// The candidates contributed by a home directory: `.config` group, then dotfile.
#[must_use]
pub fn home_group(directory: impl AsRef<Path>) -> Vec<PathBuf> {
    let home = directory.as_ref();
    let mut group = base_group(home.join(".config"));
    group.push(home.join(DOTFILE));
    group
}

/// The default XDG group, which exists only on Unix-like targets.
///
/// On other platforms `push_default_xdg` is deliberately a no-op, so the
/// expectation is empty rather than absent — callers splice this in
/// unconditionally and get the right answer on every target.
#[must_use]
pub fn default_xdg_group() -> Vec<PathBuf> {
    #[cfg(any(unix, target_os = "redox"))]
    {
        base_group("/etc/xdg")
    }
    #[cfg(not(any(unix, target_os = "redox")))]
    {
        Vec::new()
    }
}

/// The candidate contributed by the single deterministic project root.
#[must_use]
pub fn project_group() -> Vec<PathBuf> {
    vec![PathBuf::from(PROJECT_ROOT).join(DOTFILE)]
}

/// Concatenate candidate groups into one expected sequence.
#[must_use]
pub fn sequence<I>(groups: I) -> Vec<PathBuf>
where
    I: IntoIterator,
    I::Item: IntoIterator<Item = PathBuf>,
{
    groups.into_iter().flatten().collect()
}

/// Assert `actual` contains `needle` as a contiguous run, and report where.
///
/// Used where a full-list assertion would drag in an unrelated platform's
/// groups; the contiguity requirement keeps it a real ordering claim rather
/// than the containment check these helpers exist to avoid.
pub fn assert_contains_run(actual: &[PathBuf], needle: &[PathBuf]) {
    assert!(
        actual.windows(needle.len()).any(|window| window == needle),
        "expected the contiguous run {needle:?} within {actual:?}"
    );
}

/// Assert `first` appears before `second`, both being present.
pub fn assert_precedes(actual: &[PathBuf], first: &Path, second: &Path) {
    let position = |needle: &Path| actual.iter().position(|path| path == needle);
    let (Some(first_at), Some(second_at)) = (position(first), position(second)) else {
        panic!(
            "both {} and {} must be present in {actual:?}",
            first.display(),
            second.display()
        );
    };
    assert!(
        first_at < second_at,
        "{} (at {first_at}) must precede {} (at {second_at}) in {actual:?}",
        first.display(),
        second.display()
    );
}
