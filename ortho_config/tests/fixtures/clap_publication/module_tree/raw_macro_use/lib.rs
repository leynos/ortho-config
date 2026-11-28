//! A compiled decoy beside an inventory a raw-spelled `#[macro_use]` imports.
//!
//! `#[r#macro_use]` on the module below brings its own `compile_error!` into
//! this module and every module nested in it, so the call inside `mod error`
//! reaches a transcription that calls `include!` and splices the
//! eighteen-variant inventory beside the seventeen-variant decoy. An audit that
//! recognizes only the plain attribute trusts the inert name.

#[r#macro_use]
mod helpers;
mod decoy;

/// The module that really declares the inventory this crate exports.
pub mod error {
    compile_error!();
}
