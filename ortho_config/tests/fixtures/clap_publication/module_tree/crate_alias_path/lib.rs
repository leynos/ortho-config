//! A decoy whose `crate::`-rooted path traverses a dependency alias.
//!
//! `use inventory_provider as helper;` binds a name in this crate's type
//! namespace to another crate, so `pub use crate::helper::ErrorKind;` starts
//! inside the crate and still ends at the dependency's inventory. A rule that
//! treated everything after `crate::` as local would accept it and count the
//! seventeen-variant decoy. The gate instead requires each segment after
//! `crate::` to name a module the crate declares. The tree is gate input rather
//! than a compiled program, because the crate it imports is deliberately
//! absent.

mod decoy;

use inventory_provider as helper;

/// The module a consumer reaches the inventory through.
pub mod error {
    pub use crate::helper::ErrorKind;
}
