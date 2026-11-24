//! A decoy whose exported inventory is reached through a renamed re-export.
//!
//! Rust lets a re-export give an existing item a new name, so
//! `pub use crate::real::ActualErrorKind as ErrorKind;` makes the exported
//! `error::ErrorKind` an enum this walk never counts: it is declared as
//! `ActualErrorKind`, one variant longer than the file-level decoy in
//! `decoy.rs`. The count is bound to a declaration, not to the path a consumer
//! resolves, so a gate that only counted the decoy would certify the wrong
//! total. The gate refuses the renamed re-export instead. The tree is gate
//! input rather than a compiled program.

mod decoy;
mod real;

/// The module a consumer reaches the inventory through.
pub mod error {
    pub use crate::real::ActualErrorKind as ErrorKind;
}
