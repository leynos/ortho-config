//! Fixture crate root whose module hop is conditional only once a nested
//! `cfg_attr` is expanded.
//!
//! The compiler expands a `cfg_attr` that applies another `cfg_attr`, so this
//! declaration can still remove the module from the build even though no
//! immediate attribute names `cfg`.

#[cfg_attr(all(), cfg_attr(all(), cfg(any())))]
mod error;