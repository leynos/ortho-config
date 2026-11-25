//! A decoy whose explicit leaf descends through a nested module alias.
//!
//! `local::alias` resolves `alias` in `local`'s scope, where it is a `use` alias
//! for another crate, while this walk checks segments against the enclosing
//! item list. A one-segment local path reaches only the enclosing module, which
//! the walk audits; a deeper one descends into a module whose own `use` items it
//! never reads, so an explicit inventory leaf there fails closed. The tree is
//! gate input rather than a compiled program.

mod decoy;

pub mod error {
    mod local {
        pub use std::io as alias;
    }

    mod alias {}

    pub use self::local::alias::ErrorKind;
}
