//! Windows-only integration tests for `PowerShell` help output.

#[cfg(windows)]
mod tests {
    use camino::Utf8PathBuf;
    use std::error::Error;
    use std::path::PathBuf;
    use std::process::Command;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ShellCommand(String);

    impl ShellCommand {
        fn new(name: impl Into<String>) -> Self {
            Self(name.into())
        }

        fn as_str(&self) -> &str {
            &self.0
        }
    }

    impl AsRef<str> for ShellCommand {
        fn as_ref(&self) -> &str {
            self.as_str()
        }
    }

    fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        Ok(manifest_dir
            .parent()
            .ok_or("workspace root should exist")?
            .to_path_buf())
    }

    fn cargo_orthohelp_exe() -> Result<PathBuf, Box<dyn Error>> {
        let env_vars = [
            "CARGO_BIN_EXE_cargo-orthohelp",
            "CARGO_BIN_EXE_cargo_orthohelp",
            "NEXTEST_BIN_EXE_cargo-orthohelp",
            "NEXTEST_BIN_EXE_cargo_orthohelp",
        ];
        for var in env_vars {
            if let Ok(path) = std::env::var(var) {
                return Ok(PathBuf::from(path));
            }
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

    fn command_available(shell: &ShellCommand) -> bool {
        Command::new(shell.as_str())
            .arg("-NoProfile")
            .arg("-Command")
            .arg("$PSVersionTable.PSVersion.Major")
            .output()
            .is_ok()
    }

    fn run_get_help(
        shell: &ShellCommand,
        module_manifest: &Utf8PathBuf,
    ) -> Result<String, Box<dyn Error>> {
        let script = format!(
            "Import-Module -Force '{module_manifest}'; $help = Get-Help fixture -Full | Out-String; Write-Output $help"
        );
        let output = Command::new(shell.as_str())
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg(script)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("{} Get-Help failed: {stderr}", shell.as_str()).into());
        }

        Ok(decode_help_output(&output.stdout))
    }

    fn decode_help_output(bytes: &[u8]) -> String {
        if let Some(decoded) = decode_utf16(bytes, [0xFF, 0xFE], Endianness::Little) {
            return decoded;
        }
        if let Some(decoded) = decode_utf16(bytes, [0xFE, 0xFF], Endianness::Big) {
            return decoded;
        }
        if bytes.len().is_multiple_of(2) && bytes.iter().skip(1).step_by(2).all(|byte| *byte == 0) {
            let decoded = bytes
                .chunks_exact(2)
                .map(|pair| to_u16(pair, Endianness::Little))
                .collect::<Vec<_>>();
            return String::from_utf16_lossy(&decoded);
        }
        String::from_utf8_lossy(bytes).to_string()
    }

    fn decode_utf16(bytes: &[u8], bom: [u8; 2], endian: Endianness) -> Option<String> {
        let rest = bytes.strip_prefix(&bom)?;
        let decoded = rest
            .chunks_exact(2)
            .map(|pair| to_u16(pair, endian))
            .collect::<Vec<_>>();
        Some(String::from_utf16_lossy(&decoded))
    }

    #[derive(Clone, Copy)]
    enum Endianness {
        Little,
        Big,
    }

    const fn to_u16(pair: &[u8], endian: Endianness) -> u16 {
        let bytes = match pair {
            [first, second] => [*first, *second],
            _ => return 0,
        };
        match endian {
            Endianness::Little => u16::from_le_bytes(bytes),
            Endianness::Big => u16::from_be_bytes(bytes),
        }
    }

    fn ensure_contains(output: &str, needle: &str, label: &str) -> Result<(), Box<dyn Error>> {
        if output.contains(needle) {
            return Ok(());
        }
        Err(format!("missing {label} in help output").into())
    }

    fn test_get_help_full(
        shell: &ShellCommand,
        skip_if_unavailable: bool,
    ) -> Result<(), Box<dyn Error>> {
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
        test_get_help_full(&ShellCommand::new("powershell.exe"), false)
    }

    #[test]
    fn get_help_full_works_in_pwsh() -> Result<(), Box<dyn Error>> {
        test_get_help_full(&ShellCommand::new("pwsh"), true)
    }
}
