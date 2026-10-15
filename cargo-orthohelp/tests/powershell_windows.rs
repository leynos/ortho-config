//! Windows-only integration tests for `PowerShell` help output.

#[cfg(windows)]
mod fixtures;

#[cfg(windows)]
mod tests {
    //! Windows integration tests that validate generated `PowerShell` help output.

    use super::fixtures::powershell_decoding::{
        Endianness, decode_probable_utf16le_without_bom, decode_utf16,
    };
    use camino::Utf8PathBuf;
    use rstest::rstest;
    use std::error::Error;
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
            &self.0
        }
    }

    fn generate_powershell_output(
        out_dir: &Utf8PathBuf,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let exe = super::fixtures::cargo_orthohelp_exe()?;
        let root = super::fixtures::workspace_root()?;
        let output = Command::new(exe.as_str())
            .current_dir(root.as_std_path())
            .arg("orthohelp")
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

    fn is_shell_available(shell: &ShellCommand) -> bool {
        Command::new(shell.as_str())
            .arg("-NoProfile")
            .arg("-Command")
            .arg("$PSVersionTable.PSVersion.Major")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    fn run_get_help(
        shell: &ShellCommand,
        module_manifest: &Utf8PathBuf,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let escaped_manifest = module_manifest.as_str().replace('\'', "''");
        let script = format!(
            concat!(
                "$ErrorActionPreference = 'Stop'; ",
                "Import-Module -Force '{}'; ",
                "$command = Get-Command Get-Help; ",
                "$help = if ($command.Parameters.ContainsKey('UICulture')) {{ ",
                "Get-Help 'FixtureHelp\\fixture' -Full -UICulture en-US | Out-String -Width 4096 ",
                "}} else {{ ",
                "Get-Help 'FixtureHelp\\fixture' -Full | Out-String -Width 4096 ",
                "}}; ",
                "Write-Output $help"
            ),
            escaped_manifest
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
        // BOM-carrying UTF-16 first, in the order the byte-order marks
        // announce themselves, then the BOM-less guess, then UTF-8. The
        // decoders live in the shared fixture module so their buffering
        // rules stay tested on every platform, not only on Windows.
        if let Some(decoded) = decode_utf16(bytes, [0xFF, 0xFE], Endianness::Little) {
            return decoded;
        }
        if let Some(decoded) = decode_utf16(bytes, [0xFE, 0xFF], Endianness::Big) {
            return decoded;
        }
        if let Some(decoded) = decode_probable_utf16le_without_bom(bytes) {
            return decoded;
        }
        String::from_utf8_lossy(bytes).to_string()
    }

    fn ensure_contains(
        output: &str,
        needle: &str,
        label: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if output.contains(needle) {
            return Ok(());
        }
        let without_nuls = output.replace('\0', "");
        if without_nuls.contains(needle) {
            return Ok(());
        }

        let normalized_output = normalize_whitespace(&without_nuls);
        let normalized_needle = normalize_whitespace(needle);
        if normalized_output.contains(&normalized_needle) {
            return Ok(());
        }

        Err(format!(
            "missing {label} in help output.\n--- output preview ---\n{}",
            preview_output(&without_nuls, 1_200)
        )
        .into())
    }

    fn normalize_whitespace(value: &str) -> String {
        value.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn preview_output(value: &str, max_chars: usize) -> &str {
        if value.len() <= max_chars {
            return value;
        }

        let mut cutoff = max_chars;
        while !value.is_char_boundary(cutoff) {
            cutoff -= 1;
        }
        value.get(..cutoff).unwrap_or(value)
    }

    fn test_get_help_full(
        shell: &ShellCommand,
        should_skip_if_unavailable: bool,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if should_skip_if_unavailable && !is_shell_available(shell) {
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

    #[rstest]
    #[case("powershell.exe", false)]
    #[case("pwsh", true)]
    fn get_help_full_works_in_supported_shells(
        #[case] shell_name: &str,
        #[case] should_skip_if_unavailable: bool,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        test_get_help_full(&ShellCommand::new(shell_name), should_skip_if_unavailable)
    }
}
