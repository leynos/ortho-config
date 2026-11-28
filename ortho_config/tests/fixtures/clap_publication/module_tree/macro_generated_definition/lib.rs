//! A compiled decoy beside an inventory spliced in by a macro.
//!
//! A file-level scan finds `decoy.rs`, whose seventeen variants equal the
//! reviewed total, and never reads `inventory.inc`, which `include!` splices
//! into `mod error`. The contained file declares the exported inventory with
//! one variant more, so counting the decoy would report a total that describes
//! the wrong enum. Rust substitutes the items after parsing, which is why the
//! gate has to refuse the invocation rather than count around it.

mod decoy;

/// The module that really declares the inventory this crate exports.
pub mod error {
    include!("inventory.inc");
}