//! Step definitions for testing `cli_default_as_absent` attribute behaviour.
//!
//! This module verifies that non-Option fields with clap defaults are treated
//! as absent when the user did not override them on the CLI, allowing file and
//! environment configuration to take precedence.

use super::value_parsing::normalize_scalar;
use crate::fixtures::{CliDefaultArgs, CliDefaultContext, CliDefaultSources};
use anyhow::{Result, anyhow, ensure};
use clap::{CommandFactory, FromArgMatches};
use ortho_config::subcommand::Prefix;
use ortho_config::{CliValueExtractor, load_and_merge_subcommand_with_matches};
use rstest_bdd_macros::{given, then, when};
use test_helpers::figment as figment_helpers;

fn take_sources(ctx: &CliDefaultContext) -> CliDefaultSources {
    ctx.sources.take().unwrap_or_default()
}

fn update_sources<F>(ctx: &CliDefaultContext, f: F)
where
    F: FnOnce(&mut CliDefaultSources),
{
    let mut sources = ctx.sources.get_or_insert_with(CliDefaultSources::default);
    f(&mut sources);
}

/// Helper to set a source value after normalization
fn set_source_value<F>(cli_default_context: &CliDefaultContext, value: String, setter: F) -> Result<()>
where
    F: FnOnce(&mut CliDefaultSources, String),
{
    let value = normalize_scalar(&value);
    update_sources(cli_default_context, |s| {
        setter(s, value);
    });
    Ok(())
}

#[given("a clap default punctuation {value}")]
fn set_clap_default(cli_default_context: &CliDefaultContext, value: String) -> Result<()> {
    set_source_value(cli_default_context, value, |s, v| s.clap_default = Some(v))
}

#[given("a file punctuation {value}")]
fn set_file_punctuation(cli_default_context: &CliDefaultContext, value: String) -> Result<()> {
    set_source_value(cli_default_context, value, |s, v| s.file = Some(v))
}

#[given("an environment punctuation {value}")]
fn set_env_punctuation(cli_default_context: &CliDefaultContext, value: String) -> Result<()> {
    set_source_value(cli_default_context, value, |s, v| s.env = Some(v))
}

#[given("an explicit CLI punctuation {value}")]
fn set_explicit_cli_punctuation(
    cli_default_context: &CliDefaultContext,
    value: String,
) -> Result<()> {
    set_source_value(cli_default_context, value, |s, v| s.explicit_cli = Some(v))
}

#[when("the subcommand configuration is merged")]
fn merge_subcommand(cli_default_context: &CliDefaultContext) -> Result<()> {
    let sources = take_sources(cli_default_context);

    let result = figment_helpers::with_jail(|j| {
        // Write config file if provided
        if let Some(file_value) = &sources.file {
            j.create_file(
                ".app.toml",
                &format!("[cmds.greet]\npunctuation = \"{file_value}\""),
            )?;
        }

        // Set environment variable if provided
        if let Some(env_value) = &sources.env {
            j.set_env("APP_CMDS_GREET_PUNCTUATION", env_value);
        }

        // Build CLI args
        let cli_args: Vec<&str> = if let Some(explicit_value) = &sources.explicit_cli {
            vec!["greet", "--punctuation", explicit_value.as_str()]
        } else {
            vec!["greet"]
        };

        let matches = CliDefaultArgs::command().get_matches_from(cli_args);
        let args = CliDefaultArgs::from_arg_matches(&matches)
            .map_err(|err| figment::Error::from(err.to_string()))?;
        let prefix = Prefix::new("APP_");
        let merged = load_and_merge_subcommand_with_matches(&prefix, &args, &matches)
            .map_err(|err| figment::Error::from(err.to_string()))?;
        Ok(merged)
    });

    cli_default_context.merge_result.set(result);
    Ok(())
}

#[when("CLI values are extracted")]
fn extract_cli_values(cli_default_context: &CliDefaultContext) -> Result<()> {
    let sources = take_sources(cli_default_context);

    // Build CLI args based on whether explicit CLI was set
    let cli_args: Vec<&str> = if let Some(explicit_value) = &sources.explicit_cli {
        vec!["greet", "--punctuation", explicit_value.as_str()]
    } else {
        vec!["greet"]
    };

    let matches = CliDefaultArgs::command().get_matches_from(cli_args);
    let args = CliDefaultArgs::from_arg_matches(&matches)?;
    let extracted = args.extract_user_provided(&matches)?;

    cli_default_context.extracted.set(extracted);
    Ok(())
}

#[then("the resolved punctuation is {expected}")]
fn check_punctuation(cli_default_context: &CliDefaultContext, expected: String) -> Result<()> {
    let expected = normalize_scalar(&expected);
    let result = cli_default_context
        .merge_result
        .take()
        .ok_or_else(|| anyhow!("merge result unavailable"))?;
    let merged = result?;
    ensure!(
        merged.punctuation == expected,
        "expected punctuation {:?}, got {:?}",
        expected,
        merged.punctuation
    );
    Ok(())
}

fn check_punctuation_presence(
    cli_default_context: &CliDefaultContext,
    should_be_present: bool,
) -> Result<()> {
    let extracted = cli_default_context
        .extracted
        .take()
        .ok_or_else(|| anyhow!("extracted values unavailable"))?;
    let has_punctuation = extracted.get("punctuation").is_some();

    if should_be_present {
        ensure!(
            has_punctuation,
            "expected punctuation to be present, but it was absent: {:?}",
            extracted
        );
    } else {
        ensure!(
            !has_punctuation,
            "expected punctuation to be absent, but it was present: {:?}",
            extracted
        );
    }

    Ok(())
}

#[then("punctuation is absent from extracted values")]
fn check_punctuation_absent(cli_default_context: &CliDefaultContext) -> Result<()> {
    check_punctuation_presence(cli_default_context, false)
}

#[then("punctuation is present in extracted values")]
fn check_punctuation_present(cli_default_context: &CliDefaultContext) -> Result<()> {
    check_punctuation_presence(cli_default_context, true)
}
