//! Ambiguous inventory: two top-level types claim the same name.
//!
//! The gate requires exactly one definition, so a re-exported duplicate or a
//! future split cannot silently halve the count.

/// Kinds of error produced while parsing command-line arguments.
pub enum ErrorKind {
    /// A value was supplied that failed validation.
    InvalidValue,
}

/// A second definition of the same name.
pub enum ErrorKind {
    /// A value was supplied that failed validation.
    InvalidValue,
}