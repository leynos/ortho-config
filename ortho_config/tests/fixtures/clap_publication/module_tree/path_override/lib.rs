//! Fixture crate root that loads a different file from the one its `mod`
//! declaration names.
//!
//! `error.rs` sits beside this file, so a walk that resolves default file names
//! certifies it; the compiler loads `decoy.rs` instead.

#[path = "decoy.rs"]
mod error;