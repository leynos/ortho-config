//! A decoy whose exported inventory is reached through a type alias.
//!
//! `pub type ErrorKind = crate::real::ActualErrorKind;` gives the inventory
//! name to the longer enum without a `use` item, so a gate that refused only
//! renamed re-exports would still count the file-level decoy in `decoy.rs`.
//! The alias is refused wherever it appears, conditional or not, because the
//! gate cannot evaluate a condition that decides whether the compiler sees it.
//! The tree is gate input rather than a compiled program.

mod decoy;
mod real;

/// The module a consumer reaches the inventory through.
pub mod error {
    pub type ErrorKind = crate::real::ActualErrorKind;
}
