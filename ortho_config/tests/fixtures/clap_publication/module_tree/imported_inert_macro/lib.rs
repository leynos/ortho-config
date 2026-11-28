//! A compiled decoy beside an inventory an imported macro splices in.
//!
//! `compile_error!` is the one item-level macro the audit trusts by name: it
//! expands to a diagnostic and declares no item. But a `use` item can bind an
//! imported macro to that name, so the call below reaches `error_inject`, whose
//! transcription calls `include!` and splices the eighteen-variant inventory
//! beside the seventeen-variant decoy. An audit that trusts the name because no
//! local definition shadows it counts the wrong enum.

mod decoy;
mod helpers;

/// The module that really declares the inventory this crate exports.
pub mod error {
    use crate::error_inject as compile_error;

    compile_error!();
}