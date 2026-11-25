//! A decoy whose reviewed path starts at an alias for another crate.
//!
//! `pub use std::io as error;` binds the crate-root name `error` to another
//! crate, so `pub use crate::error::ErrorKind;` matches the reviewed spelling
//! while exporting `std::io::ErrorKind`. The reviewed path is trusted only when
//! its first segment after `crate` is a module the crate root declares, which an
//! alias is not. The tree is gate input rather than a compiled program.

mod decoy;

pub use std::io as error;

pub mod consumer {
    pub use crate::error::ErrorKind;
}
