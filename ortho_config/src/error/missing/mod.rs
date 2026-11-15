//! Typed payload for enumerated missing required configuration values.
//!
//! The payload owns canonical key paths and the real supply routes for each
//! absent requirement. It never contains a configuration value, so rendering a
//! report cannot leak a secret. Presentation lives here rather than in the
//! collector because display must not discover requirements, read process
//! arguments, or consult the environment.

use std::fmt;

/// The configuration key path of a missing requirement.
///
/// Segments are retained separately so a literal key containing a dot is
/// distinct from a nested path. Quoting is applied only when rendering.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConfigPath(Vec<String>);

impl ConfigPath {
    /// Build a path from ordered segments.
    pub fn new<I, S>(segments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self(segments.into_iter().map(Into::into).collect())
    }

    /// Build the empty root path.
    pub(crate) const fn root() -> Self {
        Self(Vec::new())
    }

    /// Append one segment.
    pub(crate) fn push(&mut self, segment: &str) {
        self.0.push(segment.to_owned());
    }

    /// Remove the most recently appended segment.
    pub(crate) fn pop(&mut self) {
        self.0.pop();
    }

    /// Borrow the ordered segments.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.0
    }

    /// Return the final segment, or `None` for the root path.
    pub fn last(&self) -> Option<&str> {
        self.0.last().map(String::as_str)
    }

    /// Return the number of segments.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Return true when the path has no segments.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Render the path using dots, quoting segments that need it.
    ///
    /// A segment is quoted whenever TOML would not read it as a bare key. The
    /// quoted form keeps a literal dotted key visibly distinct from a nested
    /// path with the same spelling, and it is the only spelling that names a
    /// key outside TOML's bare-key alphabet — such as one containing `=`.
    #[must_use]
    pub fn render(&self) -> String {
        self.0
            .iter()
            .map(|segment| render_segment(segment))
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl fmt::Display for ConfigPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render())
    }
}

/// Render one path segment, quoting it when a bare spelling would be ambiguous.
fn render_segment(segment: &str) -> String {
    if is_bare_key(segment) {
        return segment.to_owned();
    }
    let escaped = segment.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Report whether TOML reads `segment` as a key without quotes.
///
/// An empty segment, a dot, whitespace, and any character outside
/// `A-Za-z0-9_-` all require the quoted form, so this one predicate replaces
/// the case-by-case list of characters that happened to be considered.
fn is_bare_key(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

/// A real way a missing value can be supplied.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SupplyRoute {
    /// A command-line flag accepted by the generated parser.
    CommandLine {
        /// The long flag, including its leading dashes.
        flag: String,
        /// Whether the flag takes a value; a boolean flag does not.
        takes_value: bool,
    },
    /// An environment variable consumed by the configured provider.
    Environment {
        /// The variable name, including any prefix.
        variable: String,
    },
    /// A key in a discovered configuration file.
    ConfigurationFile {
        /// The key path as it appears in the file.
        path: ConfigPath,
        /// Whether the file must hold a table rather than a scalar.
        expects_object: bool,
    },
}

/// One missing requirement and the routes that can supply it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingRequiredValue {
    /// Canonical key path of the absent requirement.
    path: ConfigPath,
    /// Routes accepted by the real loader, in precedence order.
    routes: Vec<SupplyRoute>,
    /// Whether the requirement is an object rather than a leaf.
    expects_object: bool,
}

impl MissingRequiredValue {
    /// Build one entry from its canonical path and verified routes.
    #[must_use]
    pub const fn new(path: ConfigPath, routes: Vec<SupplyRoute>, expects_object: bool) -> Self {
        Self {
            path,
            routes,
            expects_object,
        }
    }

    /// Borrow the canonical path.
    #[must_use]
    pub const fn path(&self) -> &ConfigPath {
        &self.path
    }

    /// Borrow the accepted supply routes in precedence order.
    #[must_use]
    pub fn routes(&self) -> &[SupplyRoute] {
        &self.routes
    }

    /// Report whether the requirement is an object rather than a leaf.
    #[must_use]
    pub const fn expects_object(&self) -> bool {
        self.expects_object
    }
}

/// A non-empty, ordered report of absent required configuration values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingRequiredValues {
    /// Program label shown in the heading, when one is known.
    program: Option<String>,
    /// Missing requirements in schema declaration order.
    missing: Vec<MissingRequiredValue>,
}

impl MissingRequiredValues {
    /// Build a report, returning `None` when no requirement is missing.
    ///
    /// The fallible constructor keeps the non-empty invariant on the type
    /// rather than relying on every caller to check a list first.
    #[must_use]
    pub fn try_new<I>(program: Option<String>, missing: I) -> Option<Self>
    where
        I: IntoIterator<Item = MissingRequiredValue>,
    {
        let items: Vec<MissingRequiredValue> = missing.into_iter().collect();
        if items.is_empty() {
            None
        } else {
            Some(Self {
                program,
                missing: items,
            })
        }
    }

    /// Borrow the program label, when one was known.
    #[must_use]
    pub fn program(&self) -> Option<&str> {
        self.program.as_deref()
    }

    /// Borrow the missing requirements in schema order.
    #[must_use]
    pub fn items(&self) -> &[MissingRequiredValue] {
        &self.missing
    }

    /// Return the number of missing requirements.
    ///
    /// The value is never zero for a constructed report.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.missing.len()
    }

    /// Return whether the report is empty.
    ///
    /// Construction rejects an empty report, so this is always `false`; the
    /// accessor exists because Clippy requires it beside [`Self::len`].
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.missing.is_empty()
    }

    /// Split this report into its parts.
    #[must_use]
    pub fn into_parts(self) -> (Option<String>, Vec<MissingRequiredValue>) {
        (self.program, self.missing)
    }
}

impl std::error::Error for MissingRequiredValues {}

mod display;

#[cfg(test)]
mod tests;
