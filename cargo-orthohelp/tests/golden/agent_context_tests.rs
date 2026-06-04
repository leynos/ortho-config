//! Golden snapshot tests for agent-context JSON generation.

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use insta::{assert_snapshot, with_settings};
use std::error::Error;
use std::process::{Command, Output};
use tempfile::TempDir;

use crate::fixtures;

#[test]
fn fixture_agent_context_matches_snapshot() -> Result<(), Box<dyn Error + Send + Sync>> {
    let out_dir = tempfile::tempdir()?;
    let output = run_agent_context(&out_dir)?;
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
    let snapshot = dir.read_to_string("agent-context.json")?;
    with_settings!({snapshot_path => ".", prepend_module_to_snapshot => false}, {
        assert_snapshot!("agent_context__fixture.json", snapshot);
    });
    Ok(())
}

fn run_agent_context(out_dir: &TempDir) -> Result<Output, Box<dyn Error + Send + Sync>> {
    let exe = fixtures::cargo_orthohelp_exe()?;
    Ok(Command::new(exe.as_str())
        .current_dir(fixtures::workspace_root()?.as_std_path())
        .arg("orthohelp")
        .arg("--out-dir")
        .arg(out_dir.path())
        .arg("--format")
        .arg("agent-context")
        .arg("--package")
        .arg("orthohelp_fixture")
        .output()?)
}
