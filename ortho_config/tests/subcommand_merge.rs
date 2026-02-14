//! Tests subcommand configuration precedence (defaults < file < env < CLI) for pr and issue.
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::{OrthoConfig, load_and_merge_subcommand_for};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
use serial_test::serial;
use tempfile::TempDir;
use test_helpers::{cwd, env};

#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, Default, PartialEq)]
#[command(name = "pr")]
#[ortho_config(prefix = "VK_")]
struct PrArgs {
    #[arg(long)]
    reference: Option<String>,
    #[arg(long)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    files: Vec<String>,
}

#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, Default, PartialEq)]
#[command(name = "issue")]
#[ortho_config(prefix = "VK_")]
struct IssueArgs {
    #[arg(long)]
    reference: Option<String>,
}

#[fixture]
fn config_dir(#[default("")] cfg: &str) -> Result<(TempDir, cwd::CwdGuard)> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let cap = Dir::open_ambient_dir(dir.path(), ambient_authority()).context("open temp dir")?;
    cap.write(".vk.toml", cfg.as_bytes())
        .context("write config")?;
    let guard = cwd::set_dir(dir.path())?;
    Ok((dir, guard))
}

struct PrPrecedenceCase {
    config_content: &'static str,
    env_val: Option<&'static str>,
    cli: PrArgs,
    expected_reference: Option<&'static str>,
    expected_files: Vec<String>,
}

struct IssuePrecedenceCase {
    config_content: &'static str,
    env_val: Option<&'static str>,
    cli: IssueArgs,
    expected_reference: Option<&'static str>,
}

#[rstest]
#[case::env_over_file(
    PrPrecedenceCase {
        config_content: "[cmds.pr]\nreference = \"file_ref\"\nfiles = [\"file.txt\"]\n",
        env_val: Some("env_ref"),
        cli: PrArgs { reference: None, files: vec![] },
        expected_reference: Some("env_ref"),
        expected_files: vec!["file.txt".into()],
    },
)]
#[case::file_over_defaults(
    PrPrecedenceCase {
        config_content: "[cmds.pr]\nreference = \"file_ref\"\nfiles = [\"file.txt\"]\n",
        env_val: None,
        cli: PrArgs { reference: None, files: vec![] },
        expected_reference: Some("file_ref"),
        expected_files: vec!["file.txt".into()],
    },
)]
#[case::cli_over_env_with_file_fallback(
    PrPrecedenceCase {
        config_content: "[cmds.pr]\nreference = \"file_ref\"\nfiles = [\"file.txt\"]\n",
        env_val: Some("env_ref"),
        cli: PrArgs { reference: Some("cli_ref".into()), files: vec![] },
        expected_reference: Some("cli_ref"),
        expected_files: vec!["file.txt".into()],
    },
)]
#[serial]
fn test_pr_precedence(#[case] case: PrPrecedenceCase) -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir(case.config_content)?;
    let _guard = case
        .env_val
        .map(|val| env::set_var("VK_CMDS_PR_REFERENCE", val));
    let merged = load_and_merge_subcommand_for(&case.cli).context("merge pr args")?;
    ensure!(
        merged.reference.as_deref() == case.expected_reference,
        "expected reference {:?}, got {:?}",
        case.expected_reference,
        merged.reference
    );
    ensure!(
        merged.files == case.expected_files,
        "expected files {:?}, got {:?}",
        case.expected_files,
        merged.files
    );
    Ok(())
}

#[rstest]
#[case::env_over_file(
    IssuePrecedenceCase {
        config_content: "[cmds.issue]\nreference = \"file_ref\"\n",
        env_val: Some("env_ref"),
        cli: IssueArgs { reference: None },
        expected_reference: Some("env_ref"),
    },
)]
#[case::file_over_defaults(
    IssuePrecedenceCase {
        config_content: "[cmds.issue]\nreference = \"file_ref\"\n",
        env_val: None,
        cli: IssueArgs { reference: None },
        expected_reference: Some("file_ref"),
    },
)]
#[case::cli_over_env(
    IssuePrecedenceCase {
        config_content: "[cmds.issue]\nreference = \"file_ref\"\n",
        env_val: Some("env_ref"),
        cli: IssueArgs { reference: Some("cli_ref".into()) },
        expected_reference: Some("cli_ref"),
    },
)]
#[serial]
fn test_issue_precedence(#[case] case: IssuePrecedenceCase) -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir(case.config_content)?;
    let _guard = case
        .env_val
        .map(|val| env::set_var("VK_CMDS_ISSUE_REFERENCE", val));
    let merged = load_and_merge_subcommand_for(&case.cli).context("merge issue args")?;
    ensure!(
        merged.reference.as_deref() == case.expected_reference,
        "expected reference {:?}, got {:?}",
        case.expected_reference,
        merged.reference
    );
    Ok(())
}
