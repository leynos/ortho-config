//! A compiled decoy beside an inventory an earlier definition splices in.
//!
//! Rust resolves an unqualified `macro_rules!` call against the definition
//! textually in scope at the call, and a later redefinition does not reach
//! backwards to it. A walk that collects every definition in the item list
//! before auditing any call therefore audits this crate's call against the
//! empty redefinition below it: the call is accepted and `decoy.rs` is counted,
//! seventeen variants that equal the reviewed total, while the compiler expands
//! the earlier transcription and exports the eighteen-variant inventory.

mod decoy;

/// The module whose call must be read against the definition above it.
pub mod error {
    /// Splices the contained inventory into the enclosing module.
    macro_rules! splice {
        () => {
            include!("inventory.inc");
        };
    }

    splice!();

    /// Redefines the name after the call, which does not reach it.
    macro_rules! splice {
        () => {};
    }
}