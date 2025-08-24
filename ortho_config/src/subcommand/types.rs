//! Types used for subcommand configuration.

use crate::normalize_prefix;

/// Prefix used when constructing configuration paths and environment variables.
///
/// Stores the raw prefix as provided by the user alongside a normalised
/// lowercase version used for file lookups.
///
/// # Examples
///
/// ```rust
/// use ortho_config::subcommand::Prefix;
/// let prefix = Prefix::new("MyApp");
/// let _ = prefix;
/// ```
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Prefix {
    raw: String,
    normalized: String,
}

impl Prefix {
    /// Creates a new `Prefix` from a raw string, storing both the original and a
    /// normalised lowercase version.
    #[must_use]
    pub fn new(raw: &str) -> Self {
        Self {
            raw: raw.to_owned(),
            normalized: normalize_prefix(raw),
        }
    }

    /// Returns the normalised, lowercase form of the prefix as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.normalized
    }

    /// Returns the original, unmodified prefix string as provided by the user.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }
}

/// Name of a subcommand.
///
/// # Examples
///
/// ```rust
/// use ortho_config::subcommand::CmdName;
/// let name = CmdName::new("my-subcommand");
/// let _ = name;
/// ```
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CmdName(String);

impl CmdName {
    /// Creates a new `CmdName` from the provided raw string.
    #[must_use]
    pub fn new(raw: &str) -> Self {
        Self(raw.to_owned())
    }

    /// Returns the stored subcommand name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the subcommand name formatted as an uppercase environment variable key.
    ///
    /// Hyphens are replaced with underscores and all characters are converted to
    /// uppercase.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::subcommand::CmdName;
    /// let name = CmdName::new("my-cmd");
    /// assert_eq!(name.env_key(), "MY_CMD");
    /// ```
    #[must_use]
    pub fn env_key(&self) -> String {
        self.0.replace('-', "_").to_ascii_uppercase()
    }
}
