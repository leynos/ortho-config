//! A decoy whose `crate::` path collides with a module nested elsewhere.
//!
//! `crate::helper` resolves in the crate root's scope. A `mod helper` nested
//! inside some other module is out of that scope, so a name-only check that
//! collected module names crate-wide would treat this path as local while Rust
//! resolves it to the `std::io` alias and exports `std::io::ErrorKind`. The gate
//! therefore consults the crate root's own declarations. The tree is gate input
//! rather than a compiled program.

mod decoy;

/// A module whose nested `helper` is not visible from the crate root.
mod elsewhere {
    mod helper {}
}

use std::io as helper;

/// The module a consumer reaches the inventory through.
pub mod error {
    pub use crate::helper::ErrorKind;
}
