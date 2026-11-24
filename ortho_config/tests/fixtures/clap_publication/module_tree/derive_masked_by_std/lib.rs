//! A decoy whose procedural derive import is masked by a type import.
//!
//! A type and a derive macro occupy different namespaces, so both
//! `use inventory_macros::Inject as Debug;` and
//! `use std::fmt::Debug as Debug;` compile in one module. The later, `std`
//! rooted binding therefore cannot be read as proof that `#[derive(Debug)]`
//! invokes the compiler's derive: the procedural macro still owns the macro
//! namespace and appends its own items, including an eighteen-variant
//! `ErrorKind`, to the annotated item's scope. A gate that kept only the last
//! root per name would trust this file and count the decoy below. The tree is
//! gate input rather than a compiled program, because the crate it imports is
//! deliberately absent.

use inventory_macros::Inject as Debug;
use std::fmt::Debug as Debug;

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
