//! Tests covering `extends` handling and dependency resolution.

use super::super::*;
use super::{expect_process_extends_failure, to_anyhow, with_fresh_graph};
use crate::result_ext::ResultIntoFigment;
use anyhow::{Context, Result, anyhow, ensure};
use figment::{
    Figment,
    providers::{Format, Toml},
};
use rstest::rstest;
use std::collections::HashSet;
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
                "unexpected extends result {:?}; expected {:?}",
                ext,
                exp
            );
        }
        ExtCase::Err(msg) => match get_extends(&figment, Path::new("cfg.toml")) {
            Ok(value) => {
                return Err(anyhow!(
                    "expected extends to fail with message containing {msg}, got {:?}",
                    value
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
                "expected OrthoError::File but extends succeeded with {:?}",
                value
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

#[rstest]
#[case::relative(false)]
#[case::absolute(true)]
fn process_extends_handles_relative_and_absolute(#[case] is_abs: bool) -> Result<()> {
    with_fresh_graph(|j, root, current, visited, stack| {
        j.create_file("base.toml", "foo = \"base\"")?;
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
            "unexpected foo value {:?}",
            value
        );
        Ok(())
    })
}

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

#[test]
fn process_extends_errors_when_base_is_not_file() -> Result<()> {
    expect_process_extends_failure(
        |j, _root, _current, _visited, _stack| {
            j.create_dir("dir")?;
            Ok(Figment::from(Toml::string("extends = 'dir'")))
        },
        "expected process_extends to fail when base is not a regular file",
        "not a regular file",
    )
}

#[test]
fn process_extends_errors_when_extends_empty() -> Result<()> {
    expect_process_extends_failure(
        |j, _root, _current, _visited, _stack| {
            j.create_file("base.toml", "")?;
            Ok(Figment::from(Toml::string("extends = \"\"")))
        },
        "expected process_extends to fail when extends value is empty",
        "non-empty",
    )
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
        "unexpected foo value {:?}",
        foo
    );
    let bar = merged
        .find_value("bar")
        .context("merged figment must contain bar")?;
    ensure!(
        bar.as_str() == Some("parent"),
        "unexpected bar value {:?}",
        bar
    );
    Ok(())
}
