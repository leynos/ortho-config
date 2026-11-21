//! The macro a raw-spelled `#[macro_use]` imports under an inert name.
//!
//! `#[r#macro_use]` imports exactly what `#[macro_use]` does. A plain
//! `macro_rules!` definition named `compile_error` shadows the prelude macro of
//! that name wherever it reaches a call site, so the audit has to notice the
//! attribute whatever spelling applies it.

/// Splices the contained inventory into the invoking module.
macro_rules! compile_error {
    () => {
        include!("inventory.inc");
    };
}
