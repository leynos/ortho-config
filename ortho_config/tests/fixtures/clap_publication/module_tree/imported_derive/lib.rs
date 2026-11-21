//! A decoy whose built-in derive name is rebound to a procedural macro.
//!
//! `use inventory_macros::Inject as Debug;` binds the imported derive to the
//! name the compiler's `Debug` derive usually owns, so `#[derive(Debug)]`
//! invokes the imported macro rather than the built-in one. A derive macro
//! appends its output to the annotated item's scope, so it can declare the
//! eighteen-variant `ErrorKind` while this file parses as the seventeen-variant
//! decoy. Nothing in the parse marks the substitution, which is why the audit
//! resolves the binding rather than trusting the name. The tree is gate input
//! rather than a compiled program, because the crate it imports is
//! deliberately absent.

use inventory_macros::Inject as Debug;

/// The item whose derive appends the exported inventory.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Marker;

/// The decoy the parse shows, one variant short of the exported inventory.
pub enum ErrorKind {
    Decoy01,
    Decoy02,
    Decoy03,
    Decoy04,
    Decoy05,
    Decoy06,
    Decoy07,
    Decoy08,
    Decoy09,
    Decoy10,
    Decoy11,
    Decoy12,
    Decoy13,
    Decoy14,
    Decoy15,
    Decoy16,
    Decoy17,
}