//! The macro a `cfg_attr`-applied `#[macro_use]` imports under an inert name.
//!
//! A plain `macro_rules!` definition named `compile_error` shadows the prelude
//! macro of that name whichever way it reaches a call site. The importing
//! attribute is applied through `cfg_attr`, so the audit has to resolve the
//! attribute rather than trust its immediate spelling.

/// Splices the contained inventory into the invoking module.
macro_rules! compile_error {
    () => {
        include!("inventory.inc");
    };
}