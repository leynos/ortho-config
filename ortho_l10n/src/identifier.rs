//! One identifier convention for runtimes, derive diagnostics, and metadata.

use thiserror::Error;

/// Separator between normalized path and suffix segments.
pub const MESSAGE_ID_SEPARATOR: &str = "-";

/// Invalid input to the shared Fluent identifier convention.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdentifierError {
    /// No root segment was supplied.
    #[error("Fluent identifier must start with an ASCII letter: missing command root")]
    EmptyPath {
        /// Offending command path, which is empty.
        input: Vec<String>,
    },
    /// A path or suffix segment was empty.
    #[error("invalid Fluent identifier segment: segment must not be empty")]
    EmptySegment {
        /// Offending empty segment.
        input: String,
    },
    /// A segment contained a character outside the ASCII identifier alphabet.
    #[error("invalid Fluent identifier segment: {input:?}")]
    InvalidCharacter {
        /// Complete offending segment before normalization.
        input: String,
    },
    /// The normalized identifier did not begin with an ASCII letter.
    #[error("Fluent identifier must start with an ASCII letter: {input:?}")]
    InvalidLeadingCharacter {
        /// Complete offending normalized identifier.
        input: String,
    },
}

/// Lowercases one ASCII segment, preserving digits, hyphens, and underscores.
///
/// A segment may start with a digit when it is a fragment of a complete ID.
/// Dots belong to the caller's path syntax and are rejected here.
///
/// # Errors
///
/// Returns an empty-segment or invalid-character error containing the input.
///
/// # Examples
///
/// ```rust
/// assert_eq!(ortho_l10n::try_normalize_segment("2026_Foo")?, "2026_foo");
/// # Ok::<(), ortho_l10n::IdentifierError>(())
/// ```
pub fn try_normalize_segment(raw: &str) -> Result<String, IdentifierError> {
    if raw.is_empty() {
        return Err(IdentifierError::EmptySegment {
            input: raw.to_owned(),
        });
    }
    if !raw
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(IdentifierError::InvalidCharacter {
            input: raw.to_owned(),
        });
    }
    Ok(raw.to_ascii_lowercase())
}

/// Normalizes and joins segments into a complete Fluent message identifier.
///
/// Code generators can reuse this helper after composing their own segments.
/// All segments are validated before the leading-letter check, preserving the
/// runtime's first-error order.
///
/// # Errors
///
/// Returns [`IdentifierError`] for an empty path, invalid segment, or a root
/// that does not start with an ASCII letter.
///
/// # Examples
///
/// ```rust
/// assert_eq!(ortho_l10n::try_join_identifier(&["Demo", "123", "help"])?, "demo-123-help");
/// # Ok::<(), ortho_l10n::IdentifierError>(())
/// ```
pub fn try_join_identifier(segments: &[impl AsRef<str>]) -> Result<String, IdentifierError> {
    if segments.is_empty() {
        return Err(IdentifierError::EmptyPath { input: Vec::new() });
    }
    let normalized = segments
        .iter()
        .map(|segment| try_normalize_segment(segment.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;
    let id = normalized.join(MESSAGE_ID_SEPARATOR);
    if !id
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
    {
        return Err(IdentifierError::InvalidLeadingCharacter { input: id });
    }
    Ok(id)
}

/// Composes a command path and dotted suffix using the shared convention.
///
/// # Errors
///
/// Returns [`IdentifierError`] for missing roots, empty or invalid segments,
/// or identifiers that do not start with an ASCII letter.
///
/// # Examples
///
/// ```rust
/// assert_eq!(ortho_l10n::try_message_id_for(&["Demo", "cli"], "args.name.help")?,
///            "demo-cli-args-name-help");
/// # Ok::<(), ortho_l10n::IdentifierError>(())
/// ```
pub fn try_message_id_for(
    command_path: &[impl AsRef<str>],
    suffix: &str,
) -> Result<String, IdentifierError> {
    // Report a missing root before validating suffix segments, as the legacy
    // API did.
    if command_path.is_empty() {
        return Err(IdentifierError::EmptyPath { input: Vec::new() });
    }
    let segments = command_path
        .iter()
        .map(AsRef::as_ref)
        .chain(suffix.split('.'))
        .collect::<Vec<_>>();
    try_join_identifier(&segments)
}

/// Builds the Fluent identifier for a command path and dotted suffix.
///
/// # Panics
///
/// Panics with the existing diagnostic when the command path is empty, a
/// segment is empty or contains a character outside `[A-Za-z0-9_-]`, or the
/// normalized identifier does not start with an ASCII letter. Dynamic callers
/// can use [`try_message_id_for`] to inspect the same errors instead.
///
/// # Examples
///
/// ```rust
/// assert_eq!(ortho_l10n::message_id_for(&["hello_world", "cli"], "args.name.help"),
///            "hello_world-cli-args-name-help");
/// ```
#[must_use]
pub fn message_id_for(command_path: &[impl AsRef<str>], suffix: &str) -> String {
    match try_message_id_for(command_path, suffix) {
        Ok(identifier) => identifier,
        Err(error) => panic!("{error}"),
    }
}

#[cfg(test)]
#[path = "identifier_tests.rs"]
mod tests;
