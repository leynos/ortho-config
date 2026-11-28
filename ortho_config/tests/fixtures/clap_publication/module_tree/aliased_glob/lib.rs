//! A decoy whose reviewed glob root is reused as an alias for another crate.
//!
//! `extern crate` and `use` aliases bind a name the reviewed glob list trusts
//! by spelling, so the alias set has to include both routes. The tree is gate
//! input rather than a compiled program.

mod decoy;

pub mod error {
    use std::io as anstyle;

    pub use anstyle::*;
}
