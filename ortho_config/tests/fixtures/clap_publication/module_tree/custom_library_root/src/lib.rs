//! The default crate root this package leaves unused.
//!
//! `[lib] path = "lib/entry.rs"` moves the crate root, so Cargo never compiles
//! this file. A walk that started at the default location would count these
//! seventeen variants and report a total for source the build does not contain.

enum ErrorKind {
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