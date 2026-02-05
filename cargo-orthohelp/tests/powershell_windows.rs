//! Windows-only integration tests for `PowerShell` help output.

#[cfg(windows)]
mod tests {
    use camino::Utf8PathBuf;
    use std::error::Error;
    use std::path::PathBuf;
    use std::process::Command;

    fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        Ok(manifest_dir
            .parent()
            .ok_or("workspace root should exist")?
            .to_path_buf())
    }

    fn cargo_orthohelp_exe() -> Result<PathBuf, Box<dyn Error>> {
        if let Ok(path) = std::env::var("CARGO_BIN_EXE_cargo-orthohelp") {
            return Ok(PathBuf::from(path));
        }
        if let Ok(path) = std::env::var("CARGO_BIN_EXE_cargo_orthohelp") {
            return Ok(PathBuf::from(path));
        }
        Err("cargo-orthohelp binary path not found in environment".into())
    }

    fn generate_powershell_output(out_dir: &Utf8PathBuf) -> Result<(), Box<dyn Error>> {
        let exe = cargo_orthohelp_exe()?;
        let root = workspace_root()?;
        let output = Command::new(exe)
            .current_dir(root)
            .arg("--format")
            .arg("ps")
            .arg("--package")
            .arg("orthohelp_fixture")
            .arg("--locale")
            .arg("en-US")
            .arg("--out-dir")
            .arg(out_dir.as_str())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("cargo-orthohelp failed: {stderr}").into());
        }

        Ok(())
    }

    fn command_available(name: &str) -> bool {
        Command::new(name)
            .arg("-NoProfile")
            .arg("-Command")
            .arg("$PSVersionTable.PSVersion.Major")
            .output()
            .is_ok()
    }

    fn run_get_help(shell: &str, module_manifest: &Utf8PathBuf) -> Result<String, Box<dyn Error>> {
        let script = format!(
            "Import-Module -Force '{module_manifest}'; $help = Get-Help fixture -Full | Out-String; Write-Output $help"
        );
        let output = Command::new(shell)
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg(script)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("{shell} Get-Help failed: {stderr}").into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn ensure_contains(output: &str, needle: &str, label: &str) -> Result<(), Box<dyn Error>> {
        if output.contains(needle) {
            return Ok(());
        }
        Err(format!("missing {label} in help output").into())
    }

    fn test_get_help_full(shell: &str, skip_if_unavailable: bool) -> Result<(), Box<dyn Error>> {
        if skip_if_unavailable && !command_available(shell) {
            return Ok(());
        }
        let temp_dir = tempfile::tempdir()?;
        let out_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .map_err(|path| format!("non-UTF-8 path: {}", path.display()))?;
        generate_powershell_output(&out_dir)?;

        let module_manifest = out_dir
            .join("powershell")
            .join("FixtureHelp")
            .join("FixtureHelp.psd1");

        let output = run_get_help(shell, &module_manifest)?;
        ensure_contains(
            &output,
            "Orthohelp fixture configuration.",
            "fixture description",
        )?;
        ensure_contains(&output, "CommonParameters", "CommonParameters")?;
        Ok(())
    }

    #[test]
    fn get_help_full_works_in_windows_powershell() -> Result<(), Box<dyn Error>> {
        test_get_help_full("powershell.exe", false)
    }

    #[test]
    fn get_help_full_works_in_pwsh() -> Result<(), Box<dyn Error>> {
        test_get_help_full("pwsh", true)
    }
}
