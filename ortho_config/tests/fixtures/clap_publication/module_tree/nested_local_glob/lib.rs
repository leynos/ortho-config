//! A decoy whose internal glob descends into a nested module.
//!
//! `local::alias` resolves `alias` in `local`'s scope, where it is a `use` alias
//! for another crate, while this walk checks segments against the enclosing
//! item list. A one-segment internal glob such as `use super::*;` reaches only
//! the enclosing module and stays accepted; a deeper one descends into a module
//! whose own `use` items the walk never reads, so it fails closed. The tree is
//! gate input rather than a compiled program.

mod decoy;

pub mod error {
    mod local {
        pub use std::io as alias;
    }

    mod alias {}

    pub use self::local::alias::*;
}
