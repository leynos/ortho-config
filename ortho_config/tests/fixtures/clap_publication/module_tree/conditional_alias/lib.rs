//! A decoy whose aliasing re-export is behind a condition that holds.
//!
//! `#[cfg(all())]` always evaluates true, so the compiler sees the alias and
//! the exported name reaches the longer enum. The gate cannot evaluate a
//! condition, and refusing only unconditional aliases would let a decoy pass
//! behind one that happens to hold, so the conditional shape is refused too.
//! The tree is gate input rather than a compiled program.

mod decoy;
mod real;

/// The module a consumer reaches the inventory through.
#[cfg(all())]
pub mod error {
    pub type ErrorKind = crate::real::ActualErrorKind;
}
