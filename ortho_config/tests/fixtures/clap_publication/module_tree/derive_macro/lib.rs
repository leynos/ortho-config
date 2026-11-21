//! A decoy whose derive macro appends the inventory to its own scope.
//!
//! A derive macro receives the annotated item and appends its output beside it,
//! so `#[derive(inventory_macros::Inject)]` can declare the eighteen-variant
//! `ErrorKind` while this file parses as a seventeen-variant decoy. The
//! attribute path here is the compiler's own `derive`; the macro sits in its
//! arguments, which is the route that separates this control from the
//! attribute-path rule.

/// The decoy the parse shows, one variant short of the exported inventory.
#[derive(inventory_macros::Inject)]
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