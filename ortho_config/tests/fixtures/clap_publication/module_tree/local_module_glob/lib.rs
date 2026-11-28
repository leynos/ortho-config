//! A decoy whose reviewed glob root names a local module.
//!
//! No `use` or `extern crate` binds `anstyle`, so the imported-name check alone
//! would trust the glob. A local `mod anstyle` re-exporting `std::io` under
//! another name reaches a different inventory, so the reviewed root is also
//! voided when any item list in the crate declares that module. The tree is gate
//! input rather than a compiled program.

mod decoy;

pub mod error {
    mod anstyle {
        pub use std::io as nested;
    }

    pub use anstyle::nested::*;
}
