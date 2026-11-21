//! An honest crate whose inline module is spelled raw and declares a file.
//!
//! `mod r#outer { mod inner; }` resolves `inner` below `outer/`, because the
//! inline module's name is `outer` and not `r#outer`. A walk that joins the
//! rendered spelling looks below a directory the compiler never uses, finds no
//! file, and refuses a tree it should follow.

mod r#outer {
    mod inner;
}

/// The exported inventory, three variants.
pub enum ErrorKind {
    /// First kind.
    First,
    /// Second kind.
    Second,
    /// Third kind.
    Third,
}
