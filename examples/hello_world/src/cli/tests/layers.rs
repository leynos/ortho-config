//! Regression tests for the declarative global loader.
//!
//! These cases exercise the compose_layers-based implementation that replaces
//! the bespoke `load_global_config` helper. The assertions mirror the merge
//! order promised in the design doc: defaults, discovered files, environment,
//! then CLI.

use std::path::Path;

use super::helpers::{figment_error, parse_command_line, with_jail};
use crate::cli::{GlobalArgs, load_global_config};
use anyhow::{Result, ensure};
use ortho_config::figment;
use rstest::rstest;

#[rstest]
#[case::file_env_cli(
    &["-s", "CliSalutation", "greet"],
    r#"salutations = ["File"]"#,
    Some("HELLO_WORLD_SALUTATIONS=EnvOne,EnvTwo"),
    vec!["CliSalutation"],
)]
#[case::env_only(
    &["greet"],
    r#"salutations = ["File"]"#,
    Some("HELLO_WORLD_SALUTATIONS=EnvOnly"),
    vec!["Hello", "File", "EnvOnly"],
)]
#[case::file_only(
    &["greet"],
    r#"salutations = ["FileOnly"]"#,
    None,
    vec!["Hello", "FileOnly"],
)]
fn load_global_config_accumulates_salutations(
    #[case] cli_args: &[&str],
    #[case] file_contents: &str,
    #[case] env_var: Option<&str>,
    #[case] expected: Vec<&str>,
) -> Result<()> {
    let cli = parse_command_line(cli_args)?;
    let merged = with_jail(|jail| {
        jail.clear_env();
        jail.create_file(".hello_world.toml", file_contents)?;
        if let Some(kv) = env_var {
            let (key, value) = kv
                .split_once('=')
                .ok_or_else(|| figment::Error::from("expected key=value pair"))?;
            jail.set_env(key, value);
        }
        load_global_config(&cli.globals, None).map_err(figment_error)
    })?;

    ensure!(
        merged.salutations == expected,
        "expected layered salutations {expected:?}, got {:?}",
        merged.salutations
    );
    Ok(())
}

#[rstest]
fn load_global_config_trims_cli_salutations() -> Result<()> {
    let cli = parse_command_line(&["-s", "  Hello  ", "greet"])?;
    let merged = with_jail(|jail| {
        jail.clear_env();
        load_global_config(&cli.globals, None).map_err(figment_error)
    })?;
    ensure!(
        merged.salutations.ends_with(&[String::from("Hello")]),
        "CLI salutation should be trimmed"
    );
    Ok(())
}

#[rstest]
fn load_global_config_respects_explicit_override() -> Result<()> {
    let cli = GlobalArgs::default();
    let merged = with_jail(|jail| {
        jail.clear_env();
        jail.create_file("override.toml", r#"recipient = "Explicit""#)?;
        load_global_config(&cli, Some(Path::new("override.toml"))).map_err(figment_error)
    })?;
    ensure!(
        merged.recipient == "Explicit",
        "explicit override path should take precedence"
    );
    Ok(())
}
