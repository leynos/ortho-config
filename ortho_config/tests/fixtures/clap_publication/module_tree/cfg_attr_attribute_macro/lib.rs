//! A decoy whose inventory-replacing attribute is applied through `cfg_attr`.
//!
//! `#[cfg_attr(all(), inventory_macros::inject)]` is not spelled as the macro,
//! but the compiler expands the `cfg_attr` into that attribute — and `all()`
//! always holds — before it invokes the macro. An audit that inspected only
//! the immediate attribute path would see the compiler's own `cfg_attr` and
//! trust a seventeen-variant decoy that the expansion really replaces with the
//! eighteen-variant inventory.

/// The decoy the parse shows, one variant short of the exported inventory.
#[cfg_attr(all(), inventory_macros::inject)]
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