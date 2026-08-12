//! Profile name validation and the `ProfileName` newtype.

use std::fmt;
use std::sync::Arc;

use crate::{OrthoError, OrthoResult};

/// A validated profile name.
///
/// Names match the grammar `[A-Za-z0-9_-]+` (non-empty, case-sensitive) and
/// must not be the reserved name `default`. Defining `[profile.default]` is an
/// error; selecting `default` is treated as no selection by
/// [`SelectedProfile::resolve`](crate::profile::SelectedProfile::resolve)
/// before this type is constructed.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProfileName {
    value: String,
}

impl ProfileName {
    /// Validate `name` against the profile-name grammar.
    ///
    /// # Errors
    ///
    /// Returns [`OrthoError::InvalidProfileName`] when the name fails the
    /// grammar and [`OrthoError::ReservedProfileName`] for `default`.
    pub fn new(name: &str) -> OrthoResult<Self> {
        if name == "default" {
            return Err(Arc::new(OrthoError::ReservedProfileName {
                name: name.to_owned(),
            }));
        }
        if !name_valid(name) {
            return Err(Arc::new(OrthoError::InvalidProfileName {
                name: name.to_owned(),
            }));
        }
        Ok(Self {
            value: name.to_owned(),
        })
    }

    /// Returns the name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// Whether `name` satisfies the profile-name grammar `[A-Za-z0-9_-]+`.
#[must_use]
pub(crate) fn name_valid(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

impl fmt::Display for ProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

/// Sorted, capped list of profile names a file chain defines.
///
/// Renders the names comma-joined with a trailing "and N more" for omitted
/// entries, and reports explicitly when no
/// configuration files were found instead of an empty list — the classic
/// leaked-`<PREFIX>PROFILE` incident reads as "no configuration files were
/// found", not as a bare colon.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AvailableProfileNames {
    names: Vec<String>,
    omitted_count: usize,
}

impl AvailableProfileNames {
    /// Maximum number of names retained in the structured error payload.
    pub const DISPLAY_CAP: usize = 16;

    /// Build a sorted, capped list from `names`.
    #[must_use]
    pub fn new(mut names: Vec<String>) -> Self {
        names.sort();
        let omitted_count = names.len().saturating_sub(Self::DISPLAY_CAP);
        names.truncate(Self::DISPLAY_CAP);
        Self {
            names,
            omitted_count,
        }
    }

    /// The retained names in sorted order.
    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.names
    }
}

impl fmt::Display for AvailableProfileNames {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.names.is_empty() {
            return f.write_str("no configuration files were found");
        }
        let head = self.names.join(", ");
        if self.omitted_count == 0 {
            return f.write_str(&head);
        }
        write!(f, "{head}, and {} more", self.omitted_count)
    }
}
