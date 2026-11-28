//! An honest crate whose declaration and module hop use raw spellings.
//!
//! `pub enum r#ErrorKind` declares the name `ErrorKind`, and `mod r#helper;`
//! loads `helper.rs`, because a raw identifier differs from its plain form only
//! in spelling. Both shapes are therefore ordinary declarations the gate has to
//! count and follow, not refusals: normalising the prefix must not become a
//! blanket rejection of the spelling.

mod r#helper;

/// The exported inventory, three variants.
pub enum r#ErrorKind {
    /// First kind.
    First,
    /// Second kind.
    Second,
    /// Third kind.
    Third,
}
