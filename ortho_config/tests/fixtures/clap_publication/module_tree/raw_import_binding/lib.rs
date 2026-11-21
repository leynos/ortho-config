//! A compiled decoy beside an inventory a raw-spelled alias splices in.
//!
//! `use crate::error_inject as r#compile_error;` binds the imported macro to the
//! name `compile_error`, because the prefix does not change the identifier. An
//! audit that records the rendered spelling stores a name no call site can
//! match, trusts the prelude macro, and counts the seventeen-variant decoy while
//! `inventory.inc` declares the eighteen-variant inventory.

mod decoy;
mod helpers;

/// The module that really declares the inventory this crate exports.
pub mod error {
    use crate::error_inject as r#compile_error;

    compile_error!();
}
