//! A decoy that borrows a reviewed glob root through `extern crate`.
//!
//! `extern crate std as anstyle;` binds the reviewed root spelling to a
//! different crate, so `pub use anstyle::io::*;` exports whatever that crate
//! holds. The collector records `extern crate` aliases beside `use` aliases for
//! exactly this shape. The tree is gate input rather than a compiled program.

mod decoy;

pub mod error {
    extern crate std as anstyle;

    pub use anstyle::io::*;
}
