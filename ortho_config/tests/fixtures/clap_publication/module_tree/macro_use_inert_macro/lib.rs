//! A compiled decoy beside an inventory a `#[macro_use]` macro splices in.
//!
//! `compile_error!` is the one item-level macro the audit trusts by name, but
//! `#[macro_use]` on the module below brings its own `compile_error!` into this
//! module and every module nested in it. The call inside `mod error` therefore
//! reaches a transcription that calls `include!` and splices the
//! eighteen-variant inventory beside the seventeen-variant decoy. An audit that
//! resolved `use` items but ignored `#[macro_use]` counts the wrong enum.

#[macro_use]
mod helpers;
mod decoy;

/// The module that really declares the inventory this crate exports.
pub mod error {
    compile_error!();
}
