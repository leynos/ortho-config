//! Configuration types for the roff man page generator.

use std::fmt;

use camino::Utf8PathBuf;

/// Valid man page section number (1-8).
///
/// Man page sections are conventionally numbered 1 through 8:
/// - 1: User commands
/// - 2: System calls
/// - 3: Library functions
/// - 4: Special files
/// - 5: File formats
/// - 6: Games
/// - 7: Miscellaneous
/// - 8: System administration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManSection(u8);

impl ManSection {
    /// Creates a new `ManSection` from a raw value.
    ///
    /// # Errors
    ///
    /// Returns `InvalidManSection` if the value is not in the range 1-8.
    ///
    /// # Examples
    ///
    /// ```
    /// use cargo_orthohelp::roff::ManSection;
    ///
    /// let section = ManSection::new(1).unwrap();
    /// assert_eq!(section.as_u8(), 1);
    ///
    /// assert!(ManSection::new(0).is_err());
    /// assert!(ManSection::new(9).is_err());
    /// ```
    pub fn new(value: u8) -> Result<Self, InvalidManSection> {
        if (1..=8).contains(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidManSection(value))
        }
    }

    /// Returns the section number as a `u8`.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self.0
    }
}

impl Default for ManSection {
    fn default() -> Self {
        // Section 1 (user commands) is the standard default for CLI tools.
        Self(1)
    }
}

impl fmt::Display for ManSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Error returned when a man section number is outside the valid range (1-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidManSection(pub u8);

impl fmt::Display for InvalidManSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid man section {}: must be between 1 and 8 inclusive",
            self.0
        )
    }
}

impl std::error::Error for InvalidManSection {}

/// Configuration for roff man page generation.
#[derive(Debug, Clone)]
pub struct RoffConfig {
    /// Output directory for man pages.
    pub out_dir: Utf8PathBuf,
    /// Man page section number (1-8, default: 1 for user commands).
    pub section: ManSection,
    /// Date string for `.TH` header (format: YYYY-MM-DD or "January 2026").
    pub date: Option<String>,
    /// Whether to split subcommands into separate man pages.
    pub should_split_subcommands: bool,
    /// Optional source/version string for `.TH` header.
    pub source: Option<String>,
    /// Optional manual name for `.TH` header (for example, "User Commands").
    pub manual: Option<String>,
}

impl Default for RoffConfig {
    fn default() -> Self {
        Self {
            out_dir: Utf8PathBuf::from("man"),
            section: ManSection::default(),
            date: None,
            should_split_subcommands: false,
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
