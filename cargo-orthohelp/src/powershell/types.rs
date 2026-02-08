//! Types for `PowerShell` output generation.

use camino::Utf8PathBuf;

/// Configuration for `PowerShell` output generation.
#[derive(Debug, Clone)]
pub struct PowerShellConfig {
    /// Root output directory.
    pub out_dir: Utf8PathBuf,
    /// Module name to emit.
    pub module_name: String,
    /// Module version for the manifest.
    pub module_version: String,
    /// Wrapper function name (binary name).
    pub bin_name: String,
    /// Aliases exported by the module.
    pub export_aliases: Vec<String>,
    /// Whether to include `CommonParameters` in help output.
    pub should_include_common_parameters: bool,
    /// Whether to split subcommands into wrapper functions.
    pub should_split_subcommands: bool,
    /// Optional `HelpInfoUri` for Update-Help.
    pub help_info_uri: Option<String>,
    /// Whether to ensure an en-US help file exists.
    pub should_ensure_en_us: bool,
}

impl PowerShellConfig {
    /// Returns the module root path under the configured output directory.
    #[must_use]
    pub fn module_root(&self) -> Utf8PathBuf {
        self.out_dir.join("powershell").join(&self.module_name)
    }
}

/// Output files created by the `PowerShell` generator.
#[derive(Debug, Default, Clone)]
pub struct PowerShellOutput {
    /// Paths to all generated files.
    pub files: Vec<Utf8PathBuf>,
}

impl PowerShellOutput {
    /// Creates an empty output list.
    #[must_use]
    pub const fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// Records a generated file path.
    pub fn add_file(&mut self, path: Utf8PathBuf) {
        self.files.push(path);
    }

    /// Extends the file list with additional paths.
    pub fn extend<I>(&mut self, paths: I)
    where
        I: IntoIterator<Item = Utf8PathBuf>,
    {
        self.files.extend(paths);
    }
}
