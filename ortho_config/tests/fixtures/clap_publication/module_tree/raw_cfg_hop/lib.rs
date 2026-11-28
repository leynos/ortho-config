//! Fixture crate root whose module hop is conditional through a raw spelling.
//!
//! `#[r#cfg(..)]` is `#[cfg(..)]`: raw identifiers differ only in spelling, so
//! the compiler applies the condition. A walk that compares the attribute path
//! without removing the prefix reads the hop as unconditional and certifies a
//! file the build may exclude.

#[r#cfg(feature = "extra")]
mod error;
