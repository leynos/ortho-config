//! Declaration the compiler may never see, reached only through a `cfg_attr`
//! that applies another `cfg_attr`.

/// Fixture error kinds.
pub(crate) enum ErrorKind {
    /// First kind.
    First,
    /// Second kind.
    Second,
}