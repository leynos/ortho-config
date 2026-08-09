//! Stateless profile selection resolution.

use std::fmt;

use crate::OrthoResult;

use super::ProfileName;

/// Where a profile selection came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProfileSource {
    /// The generated `--profile` flag.
    Flag,
    /// The `<PREFIX>PROFILE` environment variable.
    Environment,
}

impl fmt::Display for ProfileSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag => f.write_str("the --profile flag"),
            Self::Environment => f.write_str("the selector environment variable"),
        }
    }
}

/// A single selected profile and where the selection came from.
///
/// At most one profile is selected per invocation in 9.1.1; the post-load
/// accessor returns a slice so multiple simultaneous profiles can arrive
/// additively later (decision D14).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedProfile {
    /// The selected profile name.
    pub name: ProfileName,
    /// How the selection was supplied.
    pub source: ProfileSource,
}

impl SelectedProfile {
    /// Resolve the selection from the flag and environment values.
    ///
    /// The flag beats the environment variable whenever it is present, even
    /// when its value is empty (a leaked `export APP_PROFILE=` suppresses the
    /// environment fallback). An empty value and the reserved name `default`
    /// both mean "no selection" (decisions D3 and D5).
    ///
    /// # Errors
    ///
    /// Returns [`crate::OrthoError::InvalidProfileName`] when the winning
    /// value fails the name grammar.
    pub fn resolve(flag: Option<&str>, env: Option<&str>) -> OrthoResult<Option<Self>> {
        if let Some(value) = flag {
            if unset(value) {
                return Ok(None);
            }
            let name = ProfileName::new(value)?;
            return Ok(Some(Self {
                name,
                source: ProfileSource::Flag,
            }));
        }
        if let Some(value) = env {
            if unset(value) {
                return Ok(None);
            }
            let name = ProfileName::new(value)?;
            return Ok(Some(Self {
                name,
                source: ProfileSource::Environment,
            }));
        }
        Ok(None)
    }
}

/// Whether a raw selector value means "no selection" (empty or `default`).
fn unset(value: &str) -> bool {
    value.is_empty() || value == "default"
}
