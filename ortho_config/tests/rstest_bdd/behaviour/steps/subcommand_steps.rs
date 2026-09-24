//! Step definitions for testing subcommand configuration loading and merging.
//!
//! This module verifies the precedence of CLI, environment, and configuration
//! file sources when loading subcommand inputs.

use super::common::SlotTakeOrExt;
use super::value_parsing::normalize_scalar;
use crate::scenario_state::{PrArgs, SubcommandContext, SubcommandSources};
use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::subcommand::{
    Prefix, SubcommandFileContext, load_and_merge_subcommand_with_sources_at,
};
use ortho_config::{MapEnv, SharedEnvSource, SharedScanEnvSource};
use rstest_bdd_macros::{given, then, when};
use std::sync::Arc;

fn take_sources(subcommand_context: &SubcommandContext) -> SubcommandSources {
    subcommand_context.sources.take().unwrap_or_default()
}

enum ReferenceField {
    Cli,
    File,
    Env,
}

impl ReferenceField {
    fn name(&self) -> &'static str {
        match self {
            Self::Cli => "CLI reference",
            Self::File => "configuration file reference",
            Self::Env => "environment reference",
        }
    }

    fn is_empty(&self, sources: &SubcommandSources) -> bool {
        match self {
            Self::Cli => sources.cli.is_none(),
            Self::File => sources.file.is_none(),
            Self::Env => sources.env.is_none(),
        }
    }

    fn assign(&self, sources: &mut SubcommandSources, value: String) {
        match self {
            Self::Cli => sources.cli = Some(value),
            Self::File => sources.file = Some(value),
            Self::Env => sources.env = Some(value),
        }
    }
}

fn set_reference(
    subcommand_context: &SubcommandContext,
    reference: String,
    field: ReferenceField,
) -> Result<()> {
    let reference = normalize_scalar(&reference);
    ensure!(
        !reference.trim().is_empty(),
        "{} must not be empty",
        field.name()
    );
    let mut sources = subcommand_context
        .sources
        .get_or_insert_with(SubcommandSources::default);
    ensure!(
        field.is_empty(&sources),
        "{} already initialised",
        field.name()
    );
    field.assign(&mut sources, reference);
    Ok(())
}

#[given("a CLI reference {reference}")]
fn set_cli_ref(subcommand_context: &SubcommandContext, reference: String) -> Result<()> {
    set_reference(subcommand_context, reference, ReferenceField::Cli)
}

#[given("no CLI reference")]
fn no_cli_ref(subcommand_context: &SubcommandContext) -> Result<()> {
    ensure!(
        subcommand_context
            .sources
            .with_ref(|sources| sources.cli.is_none())
            .unwrap_or(true),
        "CLI reference already initialised"
    );
    Ok(())
}

#[given("a configuration reference {reference}")]
fn file_ref(subcommand_context: &SubcommandContext, reference: String) -> Result<()> {
    set_reference(subcommand_context, reference, ReferenceField::File)
}

#[given("an environment reference {reference}")]
fn env_ref(subcommand_context: &SubcommandContext, reference: String) -> Result<()> {
    set_reference(subcommand_context, reference, ReferenceField::Env)
}

#[when("the subcommand configuration is loaded without defaults")]
fn load_sub(subcommand_context: &SubcommandContext) -> Result<()> {
    let sources = take_sources(subcommand_context);
    let result = if sources.is_empty() {
        PrArgs::try_parse_from(["test"]).map_err(anyhow::Error::from)
    } else {
        let cli = PrArgs {
            reference: sources.cli.clone(),
        };
        load_from_isolated_sources(&sources, &cli)?
    };
    subcommand_context.result.set(result);
    Ok(())
}

fn load_from_isolated_sources(
    sources: &SubcommandSources,
    cli: &PrArgs,
) -> Result<Result<PrArgs, anyhow::Error>> {
    let fixture_dir = tempfile::tempdir().context("create subcommand fixture directory")?;
    let fixture = Dir::open_ambient_dir(fixture_dir.path(), ambient_authority())
        .context("open subcommand fixture directory")?;
    if let Some(file_reference) = sources.file.as_ref() {
        fixture
            .write(
                ".app.toml",
                format!("[cmds.test]\nreference = \"{file_reference}\"").as_bytes(),
            )
            .context("write subcommand fixture")?;
    }

    let isolated_home = fixture_dir.path().join("home");
    let isolated_xdg_home = fixture_dir.path().join("xdg-home");
    let isolated_xdg_dirs = fixture_dir.path().join("xdg-dirs");
    let mut environment = MapEnv::new()
        .with_var("HOME", isolated_home.as_os_str())
        .with_var("XDG_CONFIG_HOME", isolated_xdg_home.as_os_str())
        .with_var("XDG_CONFIG_DIRS", isolated_xdg_dirs.as_os_str());
    if let Some(env_reference) = sources.env.as_ref() {
        environment.insert("APP_CMDS_TEST_REFERENCE", env_reference);
    }
    let source = Arc::new(environment);
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    let files = SubcommandFileContext::new(fixture_dir.path(), discovery.as_ref());
    let prefix = Prefix::new("APP_");
    let result = load_and_merge_subcommand_with_sources_at(&prefix, cli, files, merge)
        .map_err(anyhow::Error::from);
    Ok(result)
}

#[then("the merged reference is {expected}")]
fn check_ref(subcommand_context: &SubcommandContext, expected: String) -> Result<()> {
    let expected = normalize_scalar(&expected);
    let result = subcommand_context
        .result
        .take_or("subcommand result unavailable")?;
    let cfg = result?;
    ensure!(
        cfg.reference.as_deref() == Some(expected.as_str()),
        "unexpected reference {:?}; expected {:?}",
        cfg.reference,
        expected
    );
    Ok(())
}

#[then("the subcommand load fails")]
fn sub_error(subcommand_context: &SubcommandContext) -> Result<()> {
    let result = subcommand_context
        .result
        .take_or("subcommand result unavailable")?;
    ensure!(result.is_err(), "expected subcommand load to fail");
    Ok(())
}
