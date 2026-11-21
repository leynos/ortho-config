//! A compiled decoy beside an inventory in a conditionally present module.
//!
//! The condition is `cfg(all())`, which is always true, so the compiler does
//! build `mod error` and its eighteen-variant `ErrorKind` is the inventory the
//! crate exports. A gate that exempted conditional modules would skip the
//! nested declaration, count the seventeen-variant decoy, and report a total
//! that describes the wrong enum, so the gate must refuse the shape.

mod decoy;

/// The module that really declares the inventory this crate exports.
///
/// `all()` holds, so this module is compiled; the gate cannot evaluate the
/// condition and must not assume it excludes the declaration.
#[cfg(all())]
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