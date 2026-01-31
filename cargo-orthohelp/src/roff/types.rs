//! Configuration types for the roff man page generator.

use camino::Utf8PathBuf;

/// Configuration for roff man page generation.
#[derive(Debug, Clone)]
pub struct RoffConfig {
    /// Output directory for man pages.
    pub out_dir: Utf8PathBuf,
    /// Man page section number (1-8, default: 1 for user commands).
    pub section: u8,
    /// Date string for `.TH` header (format: YYYY-MM-DD or "January 2026").
    pub date: Option<String>,
    /// Whether to split subcommands into separate man pages.
    pub split_subcommands: bool,
    /// Optional source/version string for `.TH` header.
    pub source: Option<String>,
    /// Optional manual name for `.TH` header (for example, "User Commands").
    pub manual: Option<String>,
}

impl Default for RoffConfig {
    fn default() -> Self {
        Self {
            out_dir: Utf8PathBuf::from("man"),
            section: 1,
            date: None,
            split_subcommands: false,
            source: None,
            manual: None,
        }
    }
}

/// Result of generating man page(s).
#[derive(Debug, Default)]
pub struct RoffOutput {
    /// Paths to generated man page files.
    pub files: Vec<Utf8PathBuf>,
}

impl RoffOutput {
    /// Creates a new empty output.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a generated file path.
    pub fn add_file(&mut self, path: Utf8PathBuf) {
        self.files.push(path);
    }
}
