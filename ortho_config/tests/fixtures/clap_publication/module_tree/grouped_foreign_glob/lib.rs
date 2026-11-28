//! A decoy whose reviewed path is grouped with a foreign glob.
//!
//! `pub use crate::{error::ErrorKind, helper::*};` carries the reviewed path, so
//! a check that only looked for a reviewed leaf would accept the whole group
//! while the glob supplies another crate's inventory. Every leaf of a group has
//! to be reviewed, so the group fails closed. The tree is gate input rather
//! than a compiled program.

mod decoy;

pub use std::io as helper;

pub mod error {
    pub use crate::{error::ErrorKind, helper::*};
}
