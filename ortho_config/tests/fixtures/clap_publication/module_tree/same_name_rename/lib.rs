//! A decoy whose external inventory is re-exported under its own name.
//!
//! `pub use inventory_provider::ErrorKind as ErrorKind;` renames the name to
//! itself, so a check that asked only whether the binding changed anything
//! would treat it as the declaration and accept it. The root is what matters:
//! the path still reaches another crate, so the exported inventory is not the
//! one this walk counts. The tree is gate input rather than a compiled program.

mod decoy;

/// The module a consumer reaches the inventory through.
pub mod error {
    pub use inventory_provider::ErrorKind as ErrorKind;
}
