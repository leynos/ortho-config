//! An honest crate that calls a plain definition through a raw spelling.
//!
//! `r#note!` resolves to the local `macro_rules! note` because the prefix does
//! not change the identifier, so the call is the same call the plain spelling
//! makes. A lookup that compares rendered spellings finds no definition, and
//! since `note` is no inert prelude name it refuses an honest crate: this
//! fixture keeps the invocation site normalized rather than merely accepting
//! whichever spelling the definition happened to use.

/// The exported inventory, three variants.
pub enum ErrorKind {
    /// First kind.
    First,
    /// Second kind.
    Second,
    /// Third kind.
    Third,
}

/// A module whose local macro is called under its raw spelling.
pub mod error {
    /// Declares nothing, so the call contributes no item.
    macro_rules! note {
        () => {};
    }

    r#note!();
}
