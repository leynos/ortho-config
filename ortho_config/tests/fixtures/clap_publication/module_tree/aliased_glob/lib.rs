//! A decoy whose reviewed glob root is reused as an alias for another crate.
//!
//! The gate trusts the reviewed `anstyle` glob by its root spelling, so an alias
//! that borrows that spelling would let a different crate's `*` export an
//! `ErrorKind` the walk never reads. `use std::io as anstyle;` does exactly
//! that, and the gate refuses it because a name imported anywhere in the crate
//! cannot be the reviewed dependency. The tree is gate input rather than a
//! compiled program.

mod decoy;

/// The module a consumer reaches the inventory through.
pub mod error {
    use std::io as anstyle;

    pub use anstyle::*;
}
