//! Tests covering `extends` handling and dependency resolution.

use super::super::*;
use super::{to_anyhow, with_fresh_graph};
use crate::result_ext::ResultIntoFigment;
use crate::{OrthoError, OrthoResult};
use anyhow::{Context, Result, anyhow, ensure};
use figment::{
    Figment,
    providers::{Format, Toml},
};
use rstest::rstest;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

enum ExtCase {
    Ok(Option<PathBuf>),
    Err(&'static str),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum FileError {
    BaseNotAFile,
    ExtendsEmpty,
}

fn classify_file_error(err: &OrthoError) -> Option<FileError> {
    match err {
        OrthoError::File { source, .. } => {
            let io_err = source.downcast_ref::<io::Error>()?;
            match io_err.kind() {
                io::ErrorKind::InvalidInput => (io_err
                    .to_string()
                    .contains("extended path is not a regular file"))
                .then_some(FileError::BaseNotAFile),
                io::ErrorKind::InvalidData => (io_err
                    .to_string()
                    .contains("'extends' key must be a non-empty string"))
                .then_some(FileError::ExtendsEmpty),
                _ => None,
            }
        }
        _ => None,
    }
}

fn assert_extends_error(input: &str, expected: FileError) -> OrthoResult<()> {
    let tempdir = tempdir().map_err(|err| file_error(Path::new("."), err))?;
    let root = canonicalise(tempdir.path())?;
    let current = root.join("config.toml");
    fs::write(&current, input).map_err(|err| file_error(&current, err))?;

    match expected {
        FileError::BaseNotAFile => {
            let dir_path = root.join("dir");
            if !dir_path.exists() {
                fs::create_dir(&dir_path).map_err(|err| file_error(&dir_path, err))?;
            }
        }
        FileError::ExtendsEmpty => {
            let base_path = root.join("base.toml");
            if !base_path.exists() {
                fs::write(&base_path, "").map_err(|err| file_error(&base_path, err))?;
            }
        }
    }

    let figment = Figment::from(Toml::string(input));
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    let err_arc = process_extends(figment, &current, &mut visited, &mut stack)
        .expect_err("process_extends should fail for these cases");
    let actual = classify_file_error(err_arc.as_ref()).ok_or_else(|| {
        file_error(
            &current,
            io::Error::other(format!("unexpected extends error: {err_arc:?}")),
        )
    })?;

    if actual != expected {
        return Err(file_error(
            &current,
            io::Error::other(format!(
                "unexpected extends error {actual:?}; expected {expected:?}"
            )),
        ));
    }

    Ok(())
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

#[rstest]
#[case("extends = \"dir\"", FileError::BaseNotAFile)]
#[case("extends = \"\"", FileError::ExtendsEmpty)]
fn process_extends_errors(#[case] input: &str, #[case] expected: FileError) -> OrthoResult<()> {
    assert_extends_error(input, expected)
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
