//! A decoy whose attribute macro replaces the inventory it annotates.
//!
//! `#[inventory_macros::inject]` is a procedural macro: it receives the enum
//! and returns the items that replace it, so the compiler can build an
//! eighteen-variant `ErrorKind` while this file parses as the seventeen-variant
//! decoy the reviewed total already accepts. The attribute names a macro this
//! tree does not carry, so the file is gate input rather than a compiled
//! program; the replacement was proved against a scratch proc-macro crate, and
//! the gate must refuse the shape without expanding anything.

/// The decoy the parse shows, one variant short of the exported inventory.
#[inventory_macros::inject]
pub enum ErrorKind {
    Decoy01,
    Decoy02,
    Decoy03,
    Decoy04,
    Decoy05,
    Decoy06,
    Decoy07,
    Decoy08,
    Decoy09,
    Decoy10,
    Decoy11,
    Decoy12,
    Decoy13,
    Decoy14,
    Decoy15,
    Decoy16,
    Decoy17,
}