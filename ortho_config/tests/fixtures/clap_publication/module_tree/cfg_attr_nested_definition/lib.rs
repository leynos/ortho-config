//! A compiled decoy beside an inventory gated through a nested `cfg_attr`.
//!
//! The module's presence is conditional only after the compiler expands
//! `cfg_attr(all(), cfg(all()))` into `#[cfg(all())]`, which holds. A reader
//! that recognises a condition only when it sits directly on the item would
//! treat this module as unconditional and still have to refuse it; a reader
//! that resolves one level and then treats the result as unconditional would
//! not. Either way the exported eighteen-variant inventory must never be
//! replaced by the seventeen-variant decoy beside it.

mod decoy;

/// The module that really declares the inventory this crate exports.
///
/// The nested `cfg(all())` is applied only when `all()` holds, and it always
/// does, so this module is compiled and its inventory is the exported one.
#[cfg_attr(all(), cfg(all()))]
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