//! A compiled decoy beside an inventory a raw-spelled `use` path splices in.
//!
//! `use crate::helpers::r#compile_error;` binds the re-exported macro to the
//! name `compile_error`, because the prefix does not change the identifier. An
//! audit that records the rendered spelling stores a name the inert list never
//! matches, trusts the prelude macro, and counts the seventeen-variant decoy
//! while `inventory.inc` declares the eighteen-variant inventory.

mod decoy;
mod helpers;

/// The module that really declares the inventory this crate exports.
pub mod error {
    use crate::helpers::r#compile_error;

    compile_error!();
}
