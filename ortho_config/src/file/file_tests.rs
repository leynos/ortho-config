//! Tests for configuration file helpers.

use super::*;
use crate::result_ext::ResultIntoFigment;
use figment::{Figment, providers::Format, providers::Toml};
use rstest::rstest;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[deprecated(
    note = "figment::Jail::expect_with was previously deprecated; keep a wrapper so lint expectations remain stable"
)]
fn deprecated_jail_expect_with<F>(f: F)
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
{
    figment::Jail::expect_with(f);
}

fn jail_expect_with<F>(f: F)
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
{
    #[expect(deprecated, reason = "figment::Jail is used for test isolation only")]
    {
        deprecated_jail_expect_with(f);
    }
}

fn canonical_root_and_current() -> (PathBuf, PathBuf) {
    let root = canonicalise(Path::new(".")).expect("canonicalise root");
    let current = root.join("config.toml");
    (root, current)
}

fn with_fresh_graph<F>(f: F)
where
    F: FnOnce(
        &mut figment::Jail,
        &Path,
        &Path,
        &mut HashSet<PathBuf>,
        &mut Vec<PathBuf>,
    ) -> figment::error::Result<()>,
{
    jail_expect_with(|j| {
        let (root, current) = canonical_root_and_current();
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        f(j, &root, &current, &mut visited, &mut stack)
    });
}

enum ExtCase {
    Ok(Option<PathBuf>),
    Err(&'static str),
}

#[rstest]
#[case(
    "extends = \"base.toml\"",
    ExtCase::Ok(Some(PathBuf::from("base.toml")))
)]
#[case("foo = \"bar\"", ExtCase::Ok(None))]
#[case("extends = 1", ExtCase::Err("must be a string"))]
#[case("extends = \"\"", ExtCase::Err("non-empty"))]
#[case("extends = \"dir\"", ExtCase::Ok(Some(PathBuf::from("dir"))))]
fn get_extends_cases(#[case] input: &str, #[case] expected: ExtCase) {
    let figment = Figment::from(Toml::string(input));
    match expected {
        ExtCase::Ok(exp) => {
            let ext = get_extends(&figment, Path::new("cfg.toml")).expect("extends");
            assert_eq!(ext, exp);
        }
        ExtCase::Err(msg) => {
            let err = get_extends(&figment, Path::new("cfg.toml"))
                .expect_err("expected OrthoError::File for invalid 'extends'");
            assert!(err.to_string().contains(msg));
        }
    }
}

#[rstest]
#[case("extends = 1", "must be a string")]
#[case("extends = \"\"", "must be a non-empty string")]
fn get_extends_reports_error_with_file_variant(#[case] input: &str, #[case] expected_msg: &str) {
    let figment = Figment::from(Toml::string(input));
    let err = get_extends(&figment, Path::new("cfg.toml"))
        .expect_err("expected OrthoError::File for invalid 'extends'");
    match &*err {
        crate::OrthoError::File { path, source } => {
            assert!(path.ends_with("cfg.toml"), "path: {path:?}");
            assert!(
                source.to_string().contains(expected_msg),
                "source: {source}"
            );
        }
        other => panic!("expected File error, got: {other:?}"),
    }
}

#[rstest]
#[case::relative(false)]
#[case::absolute(true)]
fn resolve_base_path_resolves(#[case] is_abs: bool) {
    jail_expect_with(|j| {
        j.create_file("base.toml", "")?;
        let (root, current) = canonical_root_and_current();
        let base_path = if is_abs {
            root.join("base.toml")
        } else {
            PathBuf::from("base.toml")
        };
        let resolved = resolve_base_path(&current, base_path).to_figment()?;
        assert_eq!(resolved, root.join("base.toml"));
        Ok(())
    });
}

#[test]
fn resolve_base_path_errors_when_no_parent() {
    let err = resolve_base_path(Path::new(""), PathBuf::from("base.toml")).unwrap_err();
    assert!(
        err.to_string()
            .contains("Cannot determine parent directory")
    );
}

#[test]
fn merge_parent_child_overrides_parent_on_conflicts() {
    let parent = Figment::from(Toml::string("foo = \"parent\"\nbar = \"parent\""));
    let child = Figment::from(Toml::string("foo = \"child\""));
    let merged = merge_parent(child, parent);
    let foo = merged.find_value("foo").expect("foo");
    assert_eq!(foo.as_str(), Some("child"));
    let bar = merged.find_value("bar").expect("bar");
    assert_eq!(bar.as_str(), Some("parent"));
}

#[rstest]
#[case::relative(false)]
#[case::absolute(true)]
fn process_extends_handles_relative_and_absolute(#[case] is_abs: bool) {
    with_fresh_graph(|j, root, current, visited, stack| {
        j.create_file("base.toml", "foo = \"base\"")?;
        let config = if is_abs {
            format!("extends = '{}'", root.join("base.toml").display())
        } else {
            "extends = \"base.toml\"".to_string()
        };
        let figment = Figment::from(Toml::string(&config));
        let merged = process_extends(figment, current, visited, stack).to_figment()?;
        let value = merged.find_value("foo").expect("foo");
        assert_eq!(value.as_str(), Some("base"));
        Ok(())
    });
}

#[test]
fn process_extends_errors_when_no_parent() {
    let figment = Figment::from(Toml::string("extends = \"base.toml\""));
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    let err = process_extends(figment, Path::new(""), &mut visited, &mut stack).unwrap_err();
    assert!(
        err.to_string()
            .contains("Cannot determine parent directory")
    );
}

#[test]
fn process_extends_errors_when_base_is_not_file() {
    with_fresh_graph(|j, _root, current, visited, stack| {
        j.create_dir("dir")?;
        let figment = Figment::from(Toml::string("extends = 'dir'"));
        let err = process_extends(figment, current, visited, stack).unwrap_err();
        assert!(err.to_string().contains("not a regular file"));
        Ok(())
    });
}

#[test]
fn process_extends_errors_when_extends_empty() {
    with_fresh_graph(|j, _root, current, visited, stack| {
        j.create_file("base.toml", "")?; // placeholder to satisfy Jail
        let figment = Figment::from(Toml::string("extends = \"\""));
        let err = process_extends(figment, current, visited, stack).unwrap_err();
        assert!(err.to_string().contains("non-empty"));
        Ok(())
    });
}

#[cfg(not(any(windows, target_os = "macos")))]
#[test]
fn normalise_cycle_key_is_noop_on_case_sensitive_platforms() {
    let path = PathBuf::from("/tmp/Config.toml");
    let normalised = normalise_cycle_key(&path);
    assert_eq!(normalised, path);

    let unicode_mixed_case = PathBuf::from("/tmp/Café.toml");
    let unicode_upper_case = PathBuf::from("/tmp/CAFÉ.toml");
    assert_eq!(normalise_cycle_key(&unicode_mixed_case), unicode_mixed_case);
    assert_eq!(normalise_cycle_key(&unicode_upper_case), unicode_upper_case);

    let special_chars = PathBuf::from("/tmp/config-!@#.toml");
    assert_eq!(normalise_cycle_key(&special_chars), special_chars);

    let non_ascii = PathBuf::from("/tmp/конфиг.toml");
    assert_eq!(normalise_cycle_key(&non_ascii), non_ascii);
}

fn assert_normalise_cycle_key(
    windows_input: &str,
    windows_expected: &str,
    unix_input: &str,
    unix_expected: &str,
) {
    let (input, expected) = if cfg!(windows) {
        (
            PathBuf::from(windows_input),
            PathBuf::from(windows_expected),
        )
    } else {
        (PathBuf::from(unix_input), PathBuf::from(unix_expected))
    };
    assert_eq!(normalise_cycle_key(&input), expected);
}

#[rstest]
#[case::absolute_paths(
    r"C:\Temp\Config.toml",
    r"c:\temp\config.toml",
    "/tmp/Config.toml",
    "/tmp/config.toml"
)]
#[case::relative_paths(
    r".\Temp\Config.toml",
    r".\temp\config.toml",
    "./Temp/Config.toml",
    "./temp/config.toml"
)]
#[case::redundant_separators(
    r"C://Temp//Config.toml",
    r"c:\temp\config.toml",
    "/tmp//Nested//Config.toml",
    "/tmp/nested/config.toml"
)]
#[cfg_attr(
    not(any(windows, target_os = "macos")),
    ignore = "case-insensitive normalisation applies only on Windows and macOS"
)]
fn normalise_cycle_key_case_insensitive_scenarios(
    #[case] windows_input: &str,
    #[case] windows_expected: &str,
    #[case] unix_input: &str,
    #[case] unix_expected: &str,
) {
    assert_normalise_cycle_key(windows_input, windows_expected, unix_input, unix_expected);
}

#[test]
#[cfg_attr(
    not(any(windows, target_os = "macos")),
    ignore = "case-insensitive normalisation applies only on Windows and macOS"
)]
fn normalise_cycle_key_handles_unicode_and_special_characters() {
    if cfg!(windows) {
        let unicode = PathBuf::from(r"C:\Temp\CAFÉ.toml");
        let special = PathBuf::from(r"C:\Temp\Config-!@#.toml");
        assert_eq!(
            normalise_cycle_key(&unicode),
            PathBuf::from(r"c:\temp\CAFÉ.toml"),
        );
        assert_eq!(
            normalise_cycle_key(&special),
            PathBuf::from(r"c:\temp\config-!@#.toml"),
        );
    } else {
        let unicode = PathBuf::from("/tmp/CAFÉ.toml");
        let special = PathBuf::from("/tmp/Config-!@#.toml");
        assert_eq!(
            normalise_cycle_key(&unicode),
            PathBuf::from("/tmp/café.toml")
        );
        assert_eq!(
            normalise_cycle_key(&special),
            PathBuf::from("/tmp/config-!@#.toml"),
        );
    }
}
