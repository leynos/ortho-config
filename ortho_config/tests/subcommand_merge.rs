use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::{OrthoConfig, load_and_merge_subcommand_for};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serial_test::serial;
use tempfile::TempDir;

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

fn write_config(cfg: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    let cap = Dir::open_ambient_dir(dir.path(), ambient_authority()).expect("open temp dir");
    cap.write(".vk.toml", cfg).expect("write config");
    dir
}

struct DirGuard {
    old: Utf8PathBuf,
}

fn set_dir(dir: &TempDir) -> DirGuard {
    let old = std::env::current_dir().expect("read current dir");
    std::env::set_current_dir(dir.path()).expect("set current dir");
    DirGuard {
        old: Utf8PathBuf::from_path_buf(old).expect("utf8"),
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.old).expect("restore current dir");
    }
}

fn set_env(key: &str, val: Option<&str>) {
    unsafe {
        match val {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}

#[rstest]
#[case::env_over_file(
    "[cmds.pr]\nreference = \"file_ref\"\nfiles = [\"file.txt\"]\n",
    Some("env_ref"),
    PrArgs { reference: None, files: vec![] },
    Some("env_ref"),
    vec!["file.txt".into()],
)]
#[case::file_over_defaults(
    "[cmds.pr]\nreference = \"file_ref\"\nfiles = [\"file.txt\"]\n",
    None,
    PrArgs { reference: None, files: vec![] },
    Some("file_ref"),
    vec!["file.txt".into()],
)]
#[serial]
fn test_pr_precedence(
    #[case] config_content: &str,
    #[case] env_val: Option<&str>,
    #[case] cli: PrArgs,
    #[case] expected_reference: Option<&str>,
    #[case] expected_files: Vec<String>,
) {
    let dir = write_config(config_content);
    let _guard = set_dir(&dir);
    if let Some(val) = env_val {
        set_env("VK_CMDS_PR_REFERENCE", Some(val));
    }
    let merged = load_and_merge_subcommand_for(&cli).expect("merge pr args");
    assert_eq!(merged.reference.as_deref(), expected_reference);
    assert_eq!(merged.files, expected_files);
    if env_val.is_some() {
        set_env("VK_CMDS_PR_REFERENCE", None);
    }
}

#[rstest]
#[case::env_over_file(
    "[cmds.issue]\nreference = \"file_ref\"\n",
    Some("env_ref"),
    IssueArgs { reference: None },
    Some("env_ref"),
)]
#[case::file_over_defaults(
    "[cmds.issue]\nreference = \"file_ref\"\n",
    None,
    IssueArgs { reference: None },
    Some("file_ref"),
)]
#[serial]
fn test_issue_precedence(
    #[case] config_content: &str,
    #[case] env_val: Option<&str>,
    #[case] cli: IssueArgs,
    #[case] expected_reference: Option<&str>,
) {
    let dir = write_config(config_content);
    let _guard = set_dir(&dir);
    if let Some(val) = env_val {
        set_env("VK_CMDS_ISSUE_REFERENCE", Some(val));
    }
    let merged = load_and_merge_subcommand_for(&cli).expect("merge issue args");
    assert_eq!(merged.reference.as_deref(), expected_reference);
    if env_val.is_some() {
        set_env("VK_CMDS_ISSUE_REFERENCE", None);
    }
}
