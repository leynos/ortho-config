//! A compiled decoy beside an inventory a raw-identifier `include!` splices in.
//!
//! `r#include!` is the same macro as `include!`, and `proc-macro2` renders the
//! raw identifier with its prefix. A scanner that dropped every word holding a
//! `#` would see this wrapper's transcription as naming no macro, accept it,
//! and count the seventeen-variant decoy while `inventory.inc` declares the
//! eighteen-variant inventory this crate exports.

mod decoy;

/// The module that really declares the inventory this crate exports.
pub mod error {
    /// Splices the contained inventory into the enclosing module.
    macro_rules! wrapper {
        () => {
            r#include!("inventory.inc");
        };
    }

    wrapper!();
}