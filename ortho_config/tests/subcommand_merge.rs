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
#[serial]
fn pr_env_over_file_when_cli_absent() {
    let cfg = "[cmds.pr]\nreference = \"file_ref\"\nfiles = [\"file.txt\"]\n";
    let dir = write_config(cfg);
    let _guard = set_dir(&dir);
    set_env("VK_CMDS_PR_REFERENCE", Some("env_ref"));
    let cli = PrArgs {
        reference: None,
        files: vec![],
    };
    let merged = load_and_merge_subcommand_for(&cli).expect("merge pr args");
    assert_eq!(merged.reference.as_deref(), Some("env_ref"));
    assert_eq!(merged.files, ["file.txt"]);
    set_env("VK_CMDS_PR_REFERENCE", None);
}

#[rstest]
#[serial]
fn pr_file_over_defaults_when_env_and_cli_absent() {
    let cfg = "[cmds.pr]\nreference = \"file_ref\"\nfiles = [\"file.txt\"]\n";
    let dir = write_config(cfg);
    let _guard = set_dir(&dir);
    let cli = PrArgs {
        reference: None,
        files: vec![],
    };
    let merged = load_and_merge_subcommand_for(&cli).expect("merge pr args");
    assert_eq!(merged.reference.as_deref(), Some("file_ref"));
    assert_eq!(merged.files, ["file.txt"]);
}

#[rstest]
#[serial]
fn issue_env_over_file_when_cli_absent() {
    let cfg = "[cmds.issue]\nreference = \"file_ref\"\n";
    let dir = write_config(cfg);
    let _guard = set_dir(&dir);
    set_env("VK_CMDS_ISSUE_REFERENCE", Some("env_ref"));
    let cli = IssueArgs { reference: None };
    let merged = load_and_merge_subcommand_for(&cli).expect("merge issue args");
    assert_eq!(merged.reference.as_deref(), Some("env_ref"));
    set_env("VK_CMDS_ISSUE_REFERENCE", None);
}

#[rstest]
#[serial]
fn issue_file_over_defaults_when_env_and_cli_absent() {
    let cfg = "[cmds.issue]\nreference = \"file_ref\"\n";
    let dir = write_config(cfg);
    let _guard = set_dir(&dir);
    let cli = IssueArgs { reference: None };
    let merged = load_and_merge_subcommand_for(&cli).expect("merge issue args");
    assert_eq!(merged.reference.as_deref(), Some("file_ref"));
}
