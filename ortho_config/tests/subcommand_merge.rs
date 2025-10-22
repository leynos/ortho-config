//! Tests subcommand configuration precedence (defaults < file < env < CLI) for pr and issue.
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::{OrthoConfig, load_and_merge_subcommand_for};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
use serial_test::serial;
use std::sync::{LazyLock, Mutex, MutexGuard};
use tempfile::TempDir;
use test_helpers::env;

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

static CWD_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

struct DirGuard {
    old: Utf8PathBuf,
    _lock: MutexGuard<'static, ()>,
}

fn set_dir(dir: &TempDir) -> DirGuard {
    let lock = CWD_MUTEX.lock().expect("lock current dir");
    let old = std::env::current_dir().expect("read current dir");
    // SAFETY: Process CWD is mutated while holding CWD_MUTEX to prevent races with other tests.
    std::env::set_current_dir(dir.path()).expect("set current dir");
    DirGuard {
        old: Utf8PathBuf::from_path_buf(old).expect("utf8"),
        _lock: lock,
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        // SAFETY: Lock is still held via `_lock`, so restoration is atomic w.r.t. other tests.
        std::env::set_current_dir(&self.old).expect("restore current dir");
    }
}

#[fixture]
fn config_dir(#[default("")] cfg: &str) -> (TempDir, DirGuard) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let cap = Dir::open_ambient_dir(dir.path(), ambient_authority()).expect("open temp dir");
    cap.write(".vk.toml", cfg).expect("write config");
    let guard = set_dir(&dir);
    (dir, guard)
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
#[case::cli_over_env_with_file_fallback(
    "[cmds.pr]\nreference = \"file_ref\"\nfiles = [\"file.txt\"]\n",
    Some("env_ref"),
    PrArgs { reference: Some("cli_ref".into()), files: vec![] },
    Some("cli_ref"),
    vec!["file.txt".into()],
)]
#[serial]
fn test_pr_precedence(
    #[case] config_content: &str,
    #[case] env_val: Option<&str>,
    #[case] cli: PrArgs,
    #[case] expected_reference: Option<&str>,
    #[case] expected_files: Vec<String>,
    #[from(config_dir)]
    #[with(config_content)]
    workspace: (TempDir, DirGuard),
) {
    let (_temp_dir, _cwd_guard) = &workspace;
    let _ = config_content;
    let _env = match env_val {
        Some(val) => env::set_var("VK_CMDS_PR_REFERENCE", val),
        None => env::remove_var("VK_CMDS_PR_REFERENCE"),
    };
    let merged = load_and_merge_subcommand_for(&cli).expect("merge pr args");
    assert_eq!(merged.reference.as_deref(), expected_reference);
    assert_eq!(merged.files, expected_files);
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
#[case::cli_over_env(
    "[cmds.issue]\nreference = \"file_ref\"\n",
    Some("env_ref"),
    IssueArgs { reference: Some("cli_ref".into()) },
    Some("cli_ref"),
)]
#[serial]
fn test_issue_precedence(
    #[case] config_content: &str,
    #[case] env_val: Option<&str>,
    #[case] cli: IssueArgs,
    #[case] expected_reference: Option<&str>,
    #[from(config_dir)]
    #[with(config_content)]
    workspace: (TempDir, DirGuard),
) {
    let (_temp_dir, _cwd_guard) = &workspace;
    let _ = config_content;
    let _env = match env_val {
        Some(val) => env::set_var("VK_CMDS_ISSUE_REFERENCE", val),
        None => env::remove_var("VK_CMDS_ISSUE_REFERENCE"),
    };
    let merged = load_and_merge_subcommand_for(&cli).expect("merge issue args");
    assert_eq!(merged.reference.as_deref(), expected_reference);
}
