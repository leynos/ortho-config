//! A compiled decoy beside an inventory a shadowed `compile_error!` splices in.
//!
//! Item-level auditing trusts `compile_error!` because the standard macro
//! expands to a diagnostic and no item. Rust lets a `macro_rules!` definition
//! shadow that name, so the one defined here splices `inventory.inc` instead. A
//! gate that consults the inert list before the file's own definitions accepts
//! the call and counts `decoy.rs`, whose seventeen variants equal the reviewed
//! total, while the crate really exports the eighteen-variant inventory.

mod decoy;

/// Shadows the prelude diagnostic macro, splicing the contained inventory.
macro_rules! compile_error {
    () => {
        include!("inventory.inc");
    };
}

compile_error!();