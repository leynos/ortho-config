//! Types for `PowerShell` output generation.

use camino::Utf8PathBuf;
use std::fmt;

macro_rules! string_newtype {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            /// Creates a new typed string value.
            #[must_use]
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Returns the wrapped value.
            #[must_use]
            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_newtype!(ModuleName, "PowerShell module name.");
string_newtype!(ModuleVersion, "PowerShell module version.");
string_newtype!(BinaryName, "Executable name exposed by the wrapper.");
string_newtype!(ExportAlias, "Alias exported by the wrapper module.");
string_newtype!(
    HelpInfoUri,
    "URI used by `Update-Help` for payload discovery."
);

/// Configuration for `PowerShell` output generation.
#[derive(Debug, Clone)]
pub struct PowerShellConfig {
    /// Root output directory.
    pub out_dir: Utf8PathBuf,
    /// Module name to emit.
    pub module_name: ModuleName,
    /// Module version for the manifest.
    pub module_version: ModuleVersion,
    /// Wrapper function name (binary name).
    pub bin_name: BinaryName,
    /// Aliases exported by the module.
    pub export_aliases: Vec<ExportAlias>,
    /// Whether to include `CommonParameters` in help output.
    pub should_include_common_parameters: bool,
    /// Whether to split subcommands into wrapper functions.
    pub should_split_subcommands: bool,
    /// Optional `HelpInfoUri` for Update-Help.
    pub help_info_uri: Option<HelpInfoUri>,
    /// Whether to ensure an en-US help file exists.
    pub should_ensure_en_us: bool,
}

impl PowerShellConfig {
    /// Returns the module root path under the configured output directory.
    #[must_use]
    pub fn module_root(&self) -> Utf8PathBuf {
        self.out_dir
            .join("powershell")
            .join(self.module_name.as_ref())
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
