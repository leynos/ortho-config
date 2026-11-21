//! A compiled decoy beside an inventory a `cfg_attr`-applied macro splices in.
//!
//! `#[cfg_attr(all(), macro_use)]` is not spelled `#[macro_use]`, but the
//! condition holds, so Rust expands the attribute and imports the helper's
//! `macro_rules! compile_error` into this module and the modules nested in it.
//! An audit that recognized only the direct spelling would trust
//! `compile_error!`, count the seventeen-variant decoy, and miss the
//! eighteen-variant inventory the crate really exports.

#[cfg_attr(all(), macro_use)]
mod helpers;
mod decoy;

/// The module that really declares the inventory this crate exports.
pub mod error {
    compile_error!();
}