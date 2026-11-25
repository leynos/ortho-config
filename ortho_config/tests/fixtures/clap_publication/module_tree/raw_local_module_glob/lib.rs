//! A decoy whose reviewed glob root names a local module by raw spelling.
//!
//! Rust resolves `r#anstyle` and `anstyle` to the same module, so storing a
//! module's raw spelling while comparing normalized path segments would let
//! this local module evade the reviewed-root check. The fixture exists to prove
//! both sides are normalized through the same helper. The tree is gate input
//! rather than a compiled program.

mod decoy;

pub mod error {
    mod r#anstyle {
        pub use std::io as nested;
    }

    pub use anstyle::nested::*;
}
