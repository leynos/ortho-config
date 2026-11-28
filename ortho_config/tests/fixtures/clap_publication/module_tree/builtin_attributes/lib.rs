//! An accepted tree that uses only the compiler's own attributes.
//!
//! The attribute audit refuses every path it cannot bind to the compiler's own
//! names, so this fixture keeps the repair from becoming a rejection of ordinary
//! annotations. It carries a derive, a lint level, a documentation attribute, a
//! conditional module, and a `cfg_attr` applying the compiler's own `allow`,
//! which is the shape the reviewed `clap_builder` source really presents.
//!
//! The trait import is the second control. `use std::fmt::Debug;` binds the
//! same name as the built-in derive, and the reviewed source really imports it,
//! so the derive-binding rule must accept a binding rooted at `std`. The where
//! clause makes the import used rather than merely declared.

use std::fmt::Debug;

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

/// A rendering helper whose bound uses the imported trait.
pub fn described(kind: &ErrorKind) -> String
where
    ErrorKind: Debug,
{
    format!("{kind:?}")
}

/// A conditional module whose `cfg_attr` applies a lint level when it holds.
#[cfg_attr(all(), allow(dead_code))]
mod helpers {
    /// A constant, so the module is not empty.
    pub(crate) const REVIEWED: usize = 3;
}