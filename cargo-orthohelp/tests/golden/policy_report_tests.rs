//! Golden snapshot tests for the policy report.

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use insta::{assert_snapshot, with_settings};
use rstest::rstest;
use std::error::Error;
use std::process::{Command, Output};
use tempfile::TempDir;

use crate::fixtures;

#[rstest]
fn fixture_policy_report_matches_snapshot() -> Result<(), Box<dyn Error + Send + Sync>> {
    let out_dir = tempfile::tempdir()?;
    let output = run_policy_check(&out_dir)?;
    if !output.status.success() {
        return Err(format!(
            "cargo-orthohelp should succeed: {:?}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let out_path = Utf8PathBuf::from_path_buf(out_dir.path().to_path_buf())
        .map_err(|path| format!("non-UTF-8 output path: {}", path.display()))?;
    let dir = Dir::open_ambient_dir(&out_path, ambient_authority())?;
    let snapshot = dir.read_to_string("policy-report.json")?;
    with_settings!({snapshot_path => ".", prepend_module_to_snapshot => false}, {
        assert_snapshot!("policy_report__fixture.json", snapshot);
    });
    Ok(())
}

fn run_policy_check(out_dir: &TempDir) -> Result<Output, Box<dyn Error + Send + Sync>> {
    let exe = fixtures::cargo_orthohelp_exe()?;
    let mut command = Command::new(exe.as_str());
    command
        .current_dir(fixtures::workspace_root()?.as_std_path())
        .arg("orthohelp")
        .arg("--check-agent-native")
        .arg("--out-dir")
        .arg(out_dir.path())
        .arg("--package")
        .arg("orthohelp_fixture");
    Ok(command.output()?)
}
