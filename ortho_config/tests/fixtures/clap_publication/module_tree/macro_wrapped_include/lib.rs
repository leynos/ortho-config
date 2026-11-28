//! A compiled decoy beside an inventory a local wrapper splices in.
//!
//! This is the indirection the direct `include!` fixture does not exercise.
//! `wrapper!` is defined in this file, so its transcription is visible and
//! spells no `ErrorKind`; it calls `include!`, whose expansion this walk never
//! reads. A gate that resolves only names it can see would accept the wrapper's
//! expansion as clean and count `decoy.rs`, whose seventeen variants equal the
//! reviewed total, while `inventory.inc` declares the eighteen-variant
//! inventory this crate really exports.

mod decoy;

/// The module that really declares the inventory this crate exports.
pub mod error {
    /// Splices the contained inventory into the enclosing module.
    macro_rules! wrapper {
        () => {
            include!("inventory.inc");
        };
    }

    wrapper!();
}