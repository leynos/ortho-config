//! A compiled decoy beside an inventory declared in an inline module.
//!
//! A file-level scan finds `decoy.rs`, whose seventeen variants equal the
//! reviewed total, and never looks inside `mod error`, where the exported
//! inventory is declared with one variant more. Counting the decoy would report
//! a total that describes the wrong enum, so the gate must refuse the shape.

mod decoy;

/// The module that really declares the inventory this crate exports.
pub mod error {
    /// The exported inventory, one variant longer than the reviewed total.
    pub enum ErrorKind {
        Real01,
        Real02,
        Real03,
        Real04,
        Real05,
        Real06,
        Real07,
        Real08,
        Real09,
        Real10,
        Real11,
        Real12,
        Real13,
        Real14,
        Real15,
        Real16,
        Real17,
        Real18,
    }
}