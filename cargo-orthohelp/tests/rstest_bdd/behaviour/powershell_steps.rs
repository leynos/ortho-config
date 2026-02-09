//! PowerShell help generation step definitions.

use std::io::Read;

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest_bdd_macros::{then, when};

use super::steps::{get_out_dir, run_orthohelp, OrthoHelpContext, StepResult};

#[when("I run cargo-orthohelp with format ps for the fixture")]
fn run_with_format_ps(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let output = run_orthohelp(
        orthohelp_context,
        &[
            "--format",
            "ps",
            "--package",
            "orthohelp_fixture",
            "--locale",
            "en-US",
        ],
    )?;
    assert!(
        output.status.success(),
        "cargo-orthohelp should succeed: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    orthohelp_context.last_output.set(output);
    Ok(())
}

#[then("the output contains a PowerShell module named {module_name}")]
fn output_contains_module(
    orthohelp_context: &mut OrthoHelpContext,
    module_name: String,
) -> StepResult<()> {
    let out_root = get_out_dir(orthohelp_context)?;
    let module_root = out_root.join("powershell").join(&module_name);
    let dir = Dir::open_ambient_dir(&module_root, ambient_authority())?;

    let psm1 = Utf8PathBuf::from(format!("{module_name}.psm1"));
    dir.open(&psm1)
        .map_err(|e| format!("expected wrapper at {}: {e}", module_root.join(&psm1)))?;

    let psd1 = Utf8PathBuf::from(format!("{module_name}.psd1"));
    dir.open(&psd1)
        .map_err(|e| format!("expected manifest at {}: {e}", module_root.join(&psd1)))?;

    Ok(())
}

#[then("the PowerShell help for {module_name} includes command {command_name}")]
fn help_includes_command(
    orthohelp_context: &mut OrthoHelpContext,
    module_name: String,
    command_name: String,
) -> StepResult<()> {
    let out_root = get_out_dir(orthohelp_context)?;
    let module_root = out_root.join("powershell").join(&module_name);
    let dir = Dir::open_ambient_dir(&module_root, ambient_authority())?;
    let help_path = Utf8PathBuf::from(format!("en-US/{module_name}-help.xml"));
    let mut file = dir.open(&help_path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;

    let expected = format!("<command:name>{command_name}</command:name>");
    assert!(
        content.contains(&expected),
        "help XML should contain {expected}"
    );
    Ok(())
}

#[then("the PowerShell about topic for {module_name} exists")]
fn about_topic_exists(orthohelp_context: &mut OrthoHelpContext, module_name: String) -> StepResult<()> {
    let out_root = get_out_dir(orthohelp_context)?;
    let module_root = out_root.join("powershell").join(&module_name);
    let dir = Dir::open_ambient_dir(&module_root, ambient_authority())?;
    let about_path = Utf8PathBuf::from(format!("en-US/about_{module_name}.help.txt"));
    dir.open(&about_path)
        .map_err(|e| format!("expected about topic at {}: {e}", module_root.join(&about_path)))?;
    Ok(())
}
