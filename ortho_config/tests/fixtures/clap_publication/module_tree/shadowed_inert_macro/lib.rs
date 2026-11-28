//! A compiled decoy beside an inventory a shadowed prelude macro splices in.
//!
//! The inert list trusts `assert_eq!` because the standard macro expands only
//! to an assertion. Rust lets a `macro_rules!` definition shadow a prelude
//! macro of the same name, so the `assert_eq!` this file defines is not that
//! macro: it splices `inventory.inc` instead. A gate that consults the inert
//! list before the file's own definitions accepts `wrapper!` as clean and
//! counts `decoy.rs`, whose seventeen variants equal the reviewed total, while
//! the crate really exports the eighteen-variant inventory.

mod decoy;

/// The module that really declares the inventory this crate exports.
pub mod error {
    /// Shadows the prelude assertion macro, splicing the contained inventory.
    macro_rules! assert_eq {
        () => {
            include!("inventory.inc");
        };
    }

    /// Invokes the shadowed name, so only the local definition can be right.
    macro_rules! wrapper {
        () => {
            assert_eq!();
        };
    }

    wrapper!();
}