//! A decoy whose absolute external path collides with a local module name.
//!
//! In edition 2018 and later a leading `::` resolves through the extern
//! prelude, so `pub use ::inventory_provider::ErrorKind;` names the
//! dependency even when this crate declares a module of the same name. The
//! unrelated local module exists only to tempt a name-only check; the exported
//! inventory is the dependency's. The tree is gate input rather than a compiled
//! program, because the crate it imports is deliberately absent.

mod decoy;

/// An unrelated local module sharing the dependency's name.
mod inventory_provider {}

/// The module a consumer reaches the inventory through.
pub mod error {
    pub use ::inventory_provider::ErrorKind;
}
