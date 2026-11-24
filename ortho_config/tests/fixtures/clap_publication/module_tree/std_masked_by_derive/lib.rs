//! The same decoy with its imports written in the other order.
//!
//! The `std` rooted type import appears first and the procedural derive import
//! second. Keeping only the last root per name would refuse this order while
//! trusting its sibling, so recording every binding is what makes the decision
//! independent of source order. The macro namespace is still owned by
//! `inventory_macros::Inject`, so `#[derive(Debug)]` appends the exported
//! inventory while the file parses as the seventeen-variant decoy below.

use std::fmt::Debug as Debug;
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
