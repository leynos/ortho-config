//! Property tests for pure XDG base-directory resolution in subcommand
//! discovery.
//!
//! These keep the examples in `paths_tests.rs` while covering the whole input
//! space of `XDG_CONFIG_DIRS`: any number of absolute, relative, and empty
//! segments, in any order.
//!
//! The module is Unix-only: it is compiled solely under
//! `cfg(any(unix, target_os = "redox"))`, matching the platform that provides
//! the XDG rules it exercises. Non-Unix discovery reads neither
//! `XDG_CONFIG_DIRS` nor `XDG_CONFIG_HOME`, so there is nothing here to assert
//! elsewhere.

use super::*;
use crate::MapEnv;
use anyhow::{Context, Result, ensure};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use std::path::PathBuf;
use tempfile::TempDir;

/// A single generated `XDG_CONFIG_DIRS` segment: absolute (one or two path
/// components), relative, or empty.
#[derive(Debug, Clone)]
enum Segment {
    Absolute(String),
    Relative(String),
    Empty,
}

impl Segment {
    /// The literal text that occupies this position in the joined variable.
    fn as_os_str(&self) -> &str {
        match self {
            Self::Absolute(value) | Self::Relative(value) => value,
            Self::Empty => "",
        }
    }

    /// The base path this segment contributes, if discovery may keep it.
    fn absolute_base(&self) -> Option<PathBuf> {
        match self {
            Self::Absolute(value) => Some(PathBuf::from(value)),
            Self::Relative(_) | Self::Empty => None,
        }
    }
}

/// An absolute path of one or two components, as a bare string.
///
/// The generator never yields `..`: resolution is lexical, so a parent
/// segment would denote a base the assertion cannot describe without
/// canonicalising.
fn absolute_segment() -> impl Strategy<Value = String> {
    ("[a-z0-9]{1,8}", proptest::option::of("[a-z0-9]{1,8}")).prop_map(|(first, extra)| {
        extra.map_or_else(
            || format!("/{first}"),
            |suffix| format!("/{first}/{suffix}"),
        )
    })
}

/// One `XDG_CONFIG_DIRS` position, weighted so all three kinds appear often.
fn segment() -> impl Strategy<Value = Segment> {
    prop_oneof![
        absolute_segment().prop_map(Segment::Absolute),
        "[a-z0-9]{1,8}".prop_map(Segment::Relative),
        Just(Segment::Empty),
    ]
}

/// Between zero and seven positions, so an unset-equivalent list is reachable.
fn segments_strategy() -> impl Strategy<Value = Vec<Segment>> {
    vec(segment(), 0..8)
}

/// Turn generated segments into a source, or a failure describing the join.
///
/// Returns a `TestCaseError` rather than an `anyhow::Error`, because a
/// `proptest!` body can only `?`-convert into the former.
fn source_from_segments(segments: &[Segment]) -> std::result::Result<MapEnv, TestCaseError> {
    let values: Vec<&str> = segments.iter().map(Segment::as_os_str).collect();
    let joined = std::env::join_paths(values.iter().map(Path::new)).map_err(|error| {
        TestCaseError::fail(format!("join XDG configuration directories: {error}"))
    })?;
    Ok(MapEnv::new().with_var("XDG_CONFIG_DIRS", joined))
}

proptest! {
    /// `source_xdg_bases` keeps every absolute `XDG_CONFIG_DIRS` segment, in
    /// its original order, each joined with the discovery prefix, and falls
    /// back to `/etc/xdg/<prefix>` both when no absolute segment is present
    /// and when the variable is absent entirely.
    #[test]
    fn xdg_dirs_resolve_absolute_segments_or_default(
        segments in segments_strategy(),
        variable_is_absent in any::<bool>(),
    ) {
        let source = if variable_is_absent {
            MapEnv::new()
        } else {
            source_from_segments(&segments)?
        };

        let bases = source_xdg_bases(&Prefix::new("app"), &source);

        let expected = if variable_is_absent {
            vec![PathBuf::from("/etc/xdg/app")]
        } else {
            let absolute: Vec<PathBuf> =
                segments.iter().filter_map(Segment::absolute_base).collect();
            if absolute.is_empty() {
                vec![PathBuf::from("/etc/xdg/app")]
            } else {
                absolute.into_iter().map(|base| base.join("app")).collect()
            }
        };
        prop_assert_eq!(bases, expected);
    }

    /// An absolute `XDG_CONFIG_HOME` always leads the base list, and the
    /// prefix is appended to it exactly once.
    #[test]
    fn xdg_config_home_leads_resolved_bases(
        config_home in absolute_segment(),
        prefix in "[a-z0-9]{1,8}",
    ) {
        let source = MapEnv::new().with_var("XDG_CONFIG_HOME", &config_home);
        let bases = source_xdg_bases(&Prefix::new(&prefix), &source);
        let expected = PathBuf::from(&config_home).join(&prefix);
        prop_assert_eq!(bases.first(), Some(&expected));
    }

    /// For every generated extension group input, the candidate list contains
    /// exactly one file per extension, and each one is the first existing hit
    /// across the ordered bases.
    #[test]
    fn candidate_paths_pick_first_existing_per_extension(
        directories_with_config in vec(proptest::bool::ANY, 1..4),
    ) {
        let root = TempDir::new().map_err(|error| {
            TestCaseError::fail(format!("create root: {error}"))
        })?;
        let mut bases = Vec::new();
        for (index, has_config) in directories_with_config.iter().enumerate() {
            let base = root.path().join(format!("base{index}"));
            std::fs::create_dir_all(base.join("app")).map_err(|error| {
                TestCaseError::fail(format!("create XDG directory: {error}"))
            })?;
            if *has_config {
                std::fs::write(base.join("app/config.toml"), "").map_err(|error| {
                    TestCaseError::fail(format!("write config: {error}"))
                })?;
            }
            bases.push(base);
        }

        let joined = std::env::join_paths(&bases).map_err(|error| {
            TestCaseError::fail(format!("join XDG directories: {error}"))
        })?;
        let source = MapEnv::new().with_var("XDG_CONFIG_DIRS", joined);
        let paths = candidate_paths_at(&Prefix::new("app"), root.path(), &source)
            .map_err(|error| TestCaseError::fail(format!("candidate search failed: {error:?}")))?;

        let expected_first = directories_with_config.iter().position(|has| *has);
        let config_candidates: Vec<&PathBuf> = paths
            .iter()
            .filter(|path| path.file_name().is_some_and(|name| name == "config.toml"))
            .collect();
        match expected_first {
            Some(index) => {
                let expected = bases
                    .get(index)
                    .map(|base| base.join("app/config.toml"))
                    .expect("index comes from the generated directories");
                prop_assert_eq!(config_candidates, vec![&expected]);
            }
            None => prop_assert!(config_candidates.is_empty()),
        }
    }
}

/// A directories-with-config vector that always exercises both the "first hit"
/// and "no hit" cases would be ideal, but the property above already covers
/// both via generation; this example pins the boundary the generator can miss.
#[test]
fn candidate_paths_report_no_config_when_no_base_has_one() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let first = root.path().join("first");
    let second = root.path().join("second");
    std::fs::create_dir_all(first.join("app")).context("create first directory")?;
    std::fs::create_dir_all(second.join("app")).context("create second directory")?;
    let joined = std::env::join_paths([first.as_path(), second.as_path()])
        .context("join XDG directories")?;
    let source = MapEnv::new().with_var("XDG_CONFIG_DIRS", joined);
    let paths = candidate_paths_at(&Prefix::new("app"), root.path(), &source)?;
    ensure!(
        !paths
            .iter()
            .any(|path| path.file_name().is_some_and(|name| name == "config.toml")),
        "no base holds config.toml, so none may be reported: {paths:?}"
    );
    Ok(())
}
