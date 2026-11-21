//! Declaration behind a file-level `cfg`.
#![cfg(feature = "extra")]

/// Fixture error kinds.
pub(crate) enum ErrorKind {
    /// First kind.
    First,
}
