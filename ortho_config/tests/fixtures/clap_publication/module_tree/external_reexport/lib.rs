//! A decoy whose exported inventory is re-exported from another crate.
//!
//! `pub use inventory_provider::ErrorKind;` makes the exported name resolve to
//! the dependency's own enum, which the walk never reads, while the file-level
//! declaration in `decoy.rs` stays countable. Accepting every unrenamed `use`
//! would therefore certify the wrong inventory. The reviewed
//! `pub use kind::ErrorKind;` shape stays accepted because `kind` is one of
//! this crate's modules. The tree is gate input rather than a compiled program,
//! because the crate it imports is deliberately absent.

mod decoy;

/// The module a consumer reaches the inventory through.
pub mod error {
    pub use inventory_provider::ErrorKind;
}
