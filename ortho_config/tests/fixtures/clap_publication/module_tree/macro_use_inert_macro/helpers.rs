//! The macro `#[macro_use]` imports under an inert name.
//!
//! A plain `macro_rules!` definition named `compile_error` shadows the prelude
//! macro of that name whichever way it reaches a call site. `#[macro_use]` on
//! the declaring module is the route that needs no `use` item in the caller at
//! all, so the audit has to notice the attribute before it trusts the name.

/// Splices the contained inventory into the invoking module.
macro_rules! compile_error {
    () => {
        include!("inventory.inc");
    };
}
