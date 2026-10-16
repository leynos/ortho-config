//! Tests covering `extends` handling and dependency resolution.

use super::super::extends::{get_extends, merge_parent, process_extends};
use super::{to_anyhow, with_fresh_graph};
use crate::result_ext::ResultIntoFigment;
use anyhow::{Context, Result, anyhow, ensure};
use cap_std::fs::Dir;
use figment::{
    Figment,
    providers::{Format, Toml},
};
use rstest::rstest;
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

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
fn get_extends_cases(#[case] input: &str, #[case] expected: ExtCase) -> Result<()> {
    let figment = Figment::from(Toml::string(input));
    match expected {
        ExtCase::Ok(exp) => {
            let ext = to_anyhow(get_extends(&figment, Path::new("cfg.toml")))?;
            ensure!(
                ext == exp,
                "unexpected extends result {ext:?}; expected {exp:?}"
            );
        }
        ExtCase::Err(msg) => match get_extends(&figment, Path::new("cfg.toml")) {
            Ok(value) => {
                return Err(anyhow!(
                    "expected extends to fail with message containing {msg}, got {value:?}"
                ));
            }
            Err(err) => ensure!(
                err.to_string().contains(msg),
                "unexpected extends error {err}; expected fragment {msg}"
            ),
        },
    }
    Ok(())
}

#[rstest]
#[case("extends = 1", "must be a string")]
#[case("extends = \"\"", "must be a non-empty string")]
fn get_extends_reports_error_with_file_variant(
    #[case] input: &str,
    #[case] expected_msg: &str,
) -> Result<()> {
    let figment = Figment::from(Toml::string(input));
    let err = match get_extends(&figment, Path::new("cfg.toml")) {
        Ok(value) => {
            return Err(anyhow!(
                "expected OrthoError::File but extends succeeded with {value:?}"
            ));
        }
        Err(err) => err,
    };
    match err.as_ref() {
        crate::OrthoError::File { path, source } => {
            ensure!(path.ends_with("cfg.toml"), "unexpected file path {path:?}");
            ensure!(
                source.to_string().contains(expected_msg),
                "unexpected error source {source}"
            );
        }
        other => return Err(anyhow!("expected OrthoError::File, received {other:?}")),
    }
    Ok(())
}

/// Drive one failing `extends` case: stage fixtures, then assert the error.
///
/// `setup` receives the isolation root's [`Dir`] and returns the
/// `extends` document to load, so each case describes only the filesystem
/// state that provokes its failure. Asserting on an error *fragment* rather
/// than an exact string keeps the case coupled to the diagnosis, not to
/// incidental wording around it.
fn expect_process_extends_failure<F>(
    setup: F,
    failure_message: &str,
    expected_fragment: &str,
) -> Result<()>
where
    F: FnOnce(&Dir) -> io::Result<String>,
{
    with_fresh_graph(|dir, _root, current, visited, stack| {
        let config = setup(dir).map_err(|err| anyhow!(err))?;
        let figment = Figment::from(Toml::string(&config));
        match process_extends(figment, current, visited, stack) {
            Ok(_) => Err(anyhow!(failure_message.to_owned())),
            Err(err) => {
                ensure!(
                    err.to_string().contains(expected_fragment),
                    "unexpected extends error {err}; expected fragment {expected_fragment}"
                );
                Ok(())
            }
        }
    })
}

/// Assert both spellings of `extends` resolve to the same parent document.
///
/// A relative base is resolved against the *current file's* directory, while
/// an absolute base is taken as given. Both must land on the fixture written
/// under the isolation root, so the merged `foo` is visible either way; if
/// the relative branch ever resolved against the process working directory,
/// only the absolute case would still pass.
#[rstest]
#[case::relative(false)]
#[case::absolute(true)]
fn process_extends_handles_relative_and_absolute(#[case] is_abs: bool) -> Result<()> {
    with_fresh_graph(|dir, root, current, visited, stack| {
        dir.write("base.toml", b"foo = \"base\"")?;
        let config = if is_abs {
            format!("extends = '{}'", root.join("base.toml").display())
        } else {
            String::from("extends = \"base.toml\"")
        };
        let figment = Figment::from(Toml::string(&config));
        let merged = process_extends(figment, current, visited, stack).to_figment()?;
        let value = merged
            .find_value("foo")
            .context("merged figment must contain foo value")?;
        ensure!(
            value.as_str() == Some("base"),
            "unexpected foo value {value:?}"
        );
        Ok(())
    })
}

/// Assert a parentless current path is rejected before any filesystem work.
///
/// An empty path has no parent, so a relative `extends` cannot be anchored.
/// The loader must report that as a diagnosis rather than silently resolving
/// against the working directory or panicking on the unwrap.
#[test]
fn process_extends_errors_when_no_parent() -> Result<()> {
    let figment = Figment::from(Toml::string("extends = \"base.toml\""));
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    let Err(err) = process_extends(figment, Path::new(""), &mut visited, &mut stack) else {
        return Err(anyhow!(
            "expected process_extends to fail for path without parent directory"
        ));
    };
    ensure!(
        err.to_string()
            .contains("Cannot determine parent directory"),
        "unexpected error {err}"
    );
    Ok(())
}

/// Table of `extends` values that must be rejected, each with its own fixture.
///
/// Both cases exercise a base that is *not* a readable regular file — a
/// directory, and an empty `extends` value. They are separate rows because
/// they fail at different stages: the first reaches the filesystem and is
/// refused for the file kind, the second is rejected on the value before any
/// read is attempted.
#[rstest]
#[case::not_regular_file(
    |root: &Dir| {
        root.create_dir("dir")?;
        Ok("extends = \"dir\"".to_owned())
    },
    "expected process_extends to fail when base is not a regular file",
    "not a regular file"
)]
#[case::empty_extends(
    |root: &Dir| {
        root.write("base.toml", b"")?;
        Ok("extends = \"\"".to_owned())
    },
    "expected process_extends to fail when extends value is empty",
    "non-empty"
)]
fn process_extends_error_cases<F>(
    #[case] setup: F,
    #[case] failure_message: &str,
    #[case] expected_fragment: &str,
) -> Result<()>
where
    F: FnOnce(&Dir) -> io::Result<String>,
{
    expect_process_extends_failure(setup, failure_message, expected_fragment)
}

#[test]
fn merge_parent_child_overrides_parent_on_conflicts() -> Result<()> {
    let parent = Figment::from(Toml::string("foo = \"parent\"\nbar = \"parent\""));
    let child = Figment::from(Toml::string("foo = \"child\""));
    let merged = merge_parent(child, parent);
    let foo = merged
        .find_value("foo")
        .context("merged figment must contain foo")?;
    ensure!(
        foo.as_str() == Some("child"),
        "unexpected foo value {foo:?}"
    );
    let bar = merged
        .find_value("bar")
        .context("merged figment must contain bar")?;
    ensure!(
        bar.as_str() == Some("parent"),
        "unexpected bar value {bar:?}"
    );
    Ok(())
}
