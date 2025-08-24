//! Tests for configuration file helpers.

use super::*;
use figment::{Figment, providers::Format, providers::Toml};
use rstest::rstest;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[allow(deprecated, reason = "figment::Jail is used for test isolation only")]
fn jail_expect_with<F>(f: F)
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
{
    figment::Jail::expect_with(f);
}

fn canonical_root_and_current() -> (PathBuf, PathBuf) {
    #[cfg(windows)]
    let root = dunce::canonicalize(".").expect("canonicalise root");
    #[cfg(not(windows))]
    let root = std::fs::canonicalize(".").expect("canonicalise root");
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
            let err = get_extends(&figment, Path::new("cfg.toml")).unwrap_err();
            assert!(err.to_string().contains(msg));
        }
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
        let resolved = resolve_base_path(&current, base_path)?;
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
        let merged = process_extends(figment, current, visited, stack)?;
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
        let figment = Figment::from(Toml::string("extends = ''"));
        let err = process_extends(figment, current, visited, stack).unwrap_err();
        assert!(err.to_string().contains("non-empty"));
        Ok(())
    });
}
