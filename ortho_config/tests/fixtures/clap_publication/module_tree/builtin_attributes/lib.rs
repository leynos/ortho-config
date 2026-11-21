//! An accepted tree that uses only the compiler's own attributes.
//!
//! The attribute audit refuses every path it cannot bind to the compiler's own
//! names, so this fixture keeps the repair from becoming a rejection of ordinary
//! annotations. It carries a derive, a lint level, a documentation attribute, a
//! conditional module, and a `cfg_attr` applying the compiler's own `allow`,
//! which is the shape the reviewed `clap_builder` source really presents.

/// The inventory, annotated the way a reviewed library annotates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[allow(dead_code)]
#[doc = "The exported inventory."]
pub enum ErrorKind {
    /// First kind.
    First,
    /// Second kind.
    Second,
    /// Third kind.
    Third,
}

/// A conditional module whose `cfg_attr` applies a lint level when it holds.
#[cfg_attr(all(), allow(dead_code))]
mod helpers {
    /// A constant, so the module is not empty.
    pub(crate) const REVIEWED: usize = 3;
}