//! A decoy whose external glob may bring in an inventory of its own.
//!
//! `pub use inventory_provider::*;` has no leaf called `ErrorKind`, so a check
//! that read only the leaves would see nothing to refuse. The glob still
//! reaches whatever the dependency exports, which can include an eighteen-
//! variant inventory beside the seventeen-variant local decoy. The tree is gate
//! input rather than a compiled program.

mod decoy;

/// The module a consumer reaches the inventory through.
pub mod error {
    pub use inventory_provider::*;
}
