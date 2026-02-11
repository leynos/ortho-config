//! Windows-only integration tests for `PowerShell` help output.

#[cfg(windows)]
mod fixtures;

#[cfg(windows)]
mod tests {
    //! Windows integration tests that validate generated `PowerShell` help output.

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

    fn workspace_root() -> Result<Utf8PathBuf, Box<dyn Error>> {
        let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        Ok(manifest_dir
            .parent()
            .ok_or("workspace root should exist")?
            .to_path_buf())
    }

    fn generate_powershell_output(out_dir: &Utf8PathBuf) -> Result<(), Box<dyn Error>> {
        let exe = super::fixtures::cargo_orthohelp_exe()?;
        let root = workspace_root()?;
        let output = Command::new(exe.as_str())
            .current_dir(root.as_std_path())
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
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn run_get_help(
        shell: &ShellCommand,
        module_manifest: &Utf8PathBuf,
    ) -> Result<String, Box<dyn Error>> {
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

    fn decode_probable_utf16le_without_bom(bytes: &[u8]) -> Option<String> {
        // Windows PowerShell 5.1 can emit UTF-16LE output without a BOM when
        // piping `Get-Help ... | Out-String` through `-Command`. This fallback
        // only activates for ASCII-heavy output (many zero high-bytes) and
        // still validates the resulting UTF-16 sequence before accepting it.
        if bytes.is_empty() || !bytes.len().is_multiple_of(2) {
            return None;
        }

        let odd_byte_count = bytes.chunks_exact(2).len();
        let odd_zero_count = bytes
            .iter()
            .skip(1)
            .step_by(2)
            .filter(|byte| **byte == 0)
            .count();
        if odd_zero_count * 2 < odd_byte_count {
            return None;
        }

        let decoded = bytes
            .chunks_exact(2)
            .map(|pair| to_u16(pair, Endianness::Little))
            .collect::<Vec<_>>();
        String::from_utf16(&decoded).ok()
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
        let (first, second) = match pair {
            [first, second] => (*first as u16, *second as u16),
            _ => return 0,
        };
        match endian {
            Endianness::Little => first | (second << 8),
            Endianness::Big => (first << 8) | second,
        }
    }

    fn ensure_contains(output: &str, needle: &str, label: &str) -> Result<(), Box<dyn Error>> {
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
    ) -> Result<(), Box<dyn Error>> {
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
    ) -> Result<(), Box<dyn Error>> {
        test_get_help_full(&ShellCommand::new(shell_name), should_skip_if_unavailable)
    }
}
