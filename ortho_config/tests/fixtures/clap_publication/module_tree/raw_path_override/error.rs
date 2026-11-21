//! Declaration the compiler does not compile, because the crate root loads
//! `decoy.rs` through a raw-spelled `#[path]` instead.
//!
//! A walk that resolves default file names counts these variants as if they
//! were in the resolved build.

/// Fixture error kinds.
pub(crate) enum ErrorKind {
    /// First kind.
    First,
    /// Second kind.
    Second,
}
