//! A compiled decoy beside an inventory an imported assertion macro splices in.
//!
//! The import route is not limited to the item position. A local wrapper's
//! transcription is visible and spells no `ErrorKind`, but it invokes
//! `assert_eq!`, and a `use` item binds that name to an exported macro whose
//! transcription calls `include!`. Trusting the nested name as the prelude
//! assertion counts the seventeen-variant decoy while the eighteen-variant
//! inventory is spliced into the module.

mod decoy;
mod helpers;

/// The module that really declares the inventory this crate exports.
pub mod error {
    use crate::error_inject as assert_eq;

    /// Audited through its transcription, which invokes the imported name.
    macro_rules! wrapper {
        () => {
            assert_eq!();
        };
    }

    wrapper!();
}