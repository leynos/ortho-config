//! A decoy whose `crate::` path traverses an alias inside a nested module.
//!
//! `crate::actual::helper` resolves `helper` in `actual`'s scope, where
//! `pub use std::io as helper;` binds it to another crate. A per-segment name
//! check cannot see that, so the reviewed crate paths are a closed list and
//! this one fails closed. The tree is gate input rather than a compiled program.

mod decoy;
mod helper {}

mod actual {
    pub use std::io as helper;
}

pub mod error {
    pub use crate::actual::helper::ErrorKind;
}
