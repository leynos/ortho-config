//! A compiled decoy beside an inventory declared under a raw spelling.
//!
//! `r#ErrorKind` and `ErrorKind` are one identifier, so the declaration inside
//! `mod error` is the inventory this crate exports. A file-level scan counts
//! the seventeen-variant decoy, and a nested-shape refusal that compares the
//! rendered spelling misses the declaration it has to refuse.

mod decoy;

/// The module that really declares the inventory this crate exports.
pub mod error {
    /// The exported inventory, one variant longer than the reviewed total.
    pub enum r#ErrorKind {
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
