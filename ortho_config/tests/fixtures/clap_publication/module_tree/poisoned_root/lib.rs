//! A decoy whose compiler-rooted import reaches a local module instead.
//!
//! A `use` path rooted at `std` cannot name a procedural derive, because the
//! standard library exports none. This file rebinds the root itself: `mod std`
//! shadows the extern crate for this module, so `use std::fmt::Debug;` imports
//! whatever the local `std::fmt` re-exports, and the audit's compiler-root
//! exemption would trust a procedural macro. The derive appends its output to
//! the annotated item's scope, so the exported inventory can be one variant
//! longer than the decoy below. The sibling spelling, `extern crate … as std;`,
//! rebinds the root the same way and is covered by the declaration-class table.

mod std {
    /// The local stand-in for the standard library's formatting module.
    pub mod fmt {
        /// The derive the poisoned path reaches.
        pub use inventory_macros::Inject as Debug;
    }
}

use std::fmt::Debug;

/// The item whose derive appends the exported inventory.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Marker;

/// The decoy the parse shows, one variant short of the exported inventory.
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