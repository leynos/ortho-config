//! Tests for configuration file helpers.

use super::*;
use crate::result_ext::ResultIntoFigment;
use anyhow::{Context, Result, anyhow, ensure};
use figment::{Figment, providers::Format, providers::Toml};
use rstest::rstest;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn canonical_root_and_current() -> Result<(PathBuf, PathBuf)> {
    let root = canonicalise(Path::new("."))
        .map_err(|err| anyhow!(err.to_string()))
        .context("canonicalise configuration root directory")?;
    let current = root.join("config.toml");
    Ok((root, current))
}

fn with_fresh_graph<F>(f: F) -> Result<()>
where
    F: FnOnce(
        &mut figment::Jail,
        &Path,
        &Path,
        &mut HashSet<PathBuf>,
        &mut Vec<PathBuf>,
    ) -> Result<()>,
{
    figment::Jail::try_with(|j| {
        let (root, current) =
            canonical_root_and_current().map_err(|err| figment::Error::from(err.to_string()))?;
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        f(j, &root, &current, &mut visited, &mut stack)
            .map_err(|err| figment::Error::from(err.to_string()))
    })
    .map_err(|err| anyhow!(err.to_string()))
}

fn to_anyhow<T>(result: crate::OrthoResult<T>) -> Result<T> {
    result.map_err(|err| anyhow!(err.to_string()))
}

fn with_jail<F>(f: F) -> Result<()>
where
    F: FnOnce(&mut figment::Jail) -> Result<()>,
{
    figment::Jail::try_with(|j| f(j).map_err(|err| figment::Error::from(err.to_string())))
        .map_err(|err| anyhow!(err.to_string()))
}

fn expect_process_extends_failure<F>(
    setup: F,
    failure_message: &'static str,
    expected_fragment: &'static str,
) -> Result<()>
where
    F: FnOnce(
        &mut figment::Jail,
        &Path,
        &Path,
        &mut HashSet<PathBuf>,
        &mut Vec<PathBuf>,
    ) -> Result<Figment>,
{
    with_fresh_graph(|j, root, current, visited, stack| {
        let figment = setup(j, root, current, visited, stack)?;
        let Err(err) = process_extends(figment, current, visited, stack) else {
            return Err(anyhow!(failure_message));
        };
        ensure!(
            err.to_string().contains(expected_fragment),
            "unexpected error {err}"
        );
        Ok(())
    })
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
fn resolve_base_path_resolves(#[case] is_abs: bool) -> Result<()> {
    with_jail(|j| {
        j.create_file("base.toml", "")?;
        let (root, current) = canonical_root_and_current()?;
        let base_path = if is_abs {
            root.join("base.toml")
        } else {
            PathBuf::from("base.toml")
        };
        let resolved = resolve_base_path(&current, base_path)
            .to_figment()
            .map_err(|err| anyhow!(err.to_string()))?;
        ensure!(
            resolved == root.join("base.toml"),
            "unexpected resolved path {:?}",
            resolved
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn resolve_base_path_errors_when_no_parent() -> Result<()> {
    let err = match resolve_base_path(Path::new(""), PathBuf::from("base.toml")) {
        Ok(path) => {
            return Err(anyhow!(
                "expected resolve_base_path to fail for path without parent, got {:?}",
                path
            ));
        }
        Err(err) => err,
    };
    ensure!(
        err.to_string()
            .contains("Cannot determine parent directory"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[rstest]
#[case::relative(false)]
#[case::absolute(true)]
fn resolve_base_path_reports_missing_file(#[case] is_abs: bool) -> Result<()> {
    with_jail(|_| {
        let (root, current) = canonical_root_and_current()?;
        let expected_base = root.join("missing.toml");
        let base = if is_abs {
            expected_base.clone()
        } else {
            PathBuf::from("missing.toml")
        };
        let err = match resolve_base_path(&current, base) {
            Ok(path) => {
                return Err(anyhow!(
                    "expected resolve_base_path to fail for missing file, got {:?}",
                    path
                ));
            }
            Err(err) => err,
        };
        let msg = err.to_string();
        ensure!(
            msg.contains(expected_base.to_string_lossy().as_ref()),
            "error message {msg} does not mention missing base {expected_base:?}"
        );
        ensure!(
            msg.contains(current.to_string_lossy().as_ref()),
            "error message {msg} does not mention current path"
        );
        ensure!(msg.contains("does not exist"), "message: {msg}");
        match err.as_ref() {
            crate::OrthoError::File { source, .. } => {
                let io_err = source
                    .downcast_ref::<std::io::Error>()
                    .ok_or_else(|| anyhow!("expected std::io::Error source"))?;
                ensure!(
                    io_err.kind() == std::io::ErrorKind::NotFound,
                    "unexpected IO error kind: {:?}",
                    io_err.kind()
                );
            }
            other => {
                return Err(anyhow!("expected File error, received {other:?}"));
            }
        }
        Ok(())
    })
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
    })?;
    Ok(())
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

#[cfg(not(any(windows, target_os = "macos")))]
#[test]
fn normalise_cycle_key_is_noop_on_case_sensitive_platforms() -> Result<()> {
    let path = PathBuf::from("/tmp/Config.toml");
    let normalised = normalise_cycle_key(&path);
    ensure!(
        normalised == path,
        "expected {:?}, got {:?}",
        path,
        normalised
    );

    let unicode_mixed_case = PathBuf::from("/tmp/Café.toml");
    let unicode_upper_case = PathBuf::from("/tmp/CAFÉ.toml");
    ensure!(
        normalise_cycle_key(&unicode_mixed_case) == unicode_mixed_case,
        "unicode path normalisation changed value"
    );
    ensure!(
        normalise_cycle_key(&unicode_upper_case) == unicode_upper_case,
        "unicode uppercase path normalisation changed value"
    );

    let special_chars = PathBuf::from("/tmp/config-!@#.toml");
    ensure!(
        normalise_cycle_key(&special_chars) == special_chars,
        "special character path normalisation changed value"
    );

    let non_ascii = PathBuf::from("/tmp/конфиг.toml");
    ensure!(
        normalise_cycle_key(&non_ascii) == non_ascii,
        "Cyrillic path normalisation changed value"
    );
    Ok(())
}

fn assert_normalise_cycle_key(
    windows_input: &str,
    windows_expected: &str,
    unix_input: &str,
    unix_expected: &str,
) -> Result<()> {
    let (input, expected) = if cfg!(windows) {
        (
            PathBuf::from(windows_input),
            PathBuf::from(windows_expected),
        )
    } else {
        (PathBuf::from(unix_input), PathBuf::from(unix_expected))
    };
    ensure!(
        normalise_cycle_key(&input) == expected,
        "normalised path mismatch for input {:?}",
        input
    );
    Ok(())
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
) -> Result<()> {
    assert_normalise_cycle_key(windows_input, windows_expected, unix_input, unix_expected)?;
    Ok(())
}

#[test]
#[cfg_attr(
    not(any(windows, target_os = "macos")),
    ignore = "case-insensitive normalisation applies only on Windows and macOS"
)]
fn normalise_cycle_key_handles_unicode_and_special_characters() -> Result<()> {
    if cfg!(windows) {
        let unicode = PathBuf::from(r"C:\Temp\CAFÉ.toml");
        let special = PathBuf::from(r"C:\Temp\Config-!@#.toml");
        ensure!(
            normalise_cycle_key(&unicode) == Path::new(r"c:\temp\CAFÉ.toml"),
            "unexpected unicode normalisation"
        );
        ensure!(
            normalise_cycle_key(&special) == Path::new(r"c:\temp\config-!@#.toml"),
            "unexpected special character normalisation"
        );
    } else {
        let unicode = PathBuf::from("/tmp/CAFÉ.toml");
        let special = PathBuf::from("/tmp/Config-!@#.toml");
        ensure!(
            normalise_cycle_key(&unicode) == Path::new("/tmp/café.toml"),
            "unexpected unicode normalisation"
        );
        ensure!(
            normalise_cycle_key(&special) == Path::new("/tmp/config-!@#.toml"),
            "unexpected special character normalisation"
        );
    }
    Ok(())
}
