//! Fixture crate root that imports another name beside an inert macro.
//!
//! The audit trusts `compile_error!` only where no `use` item in the module
//! could have bound the name. An ordinary import of an unrelated name must not
//! make every inert name ambiguous, or an honest crate would be refused for
//! importing anything at all. This tree mirrors the shape the real crate
//! audits: a conditional diagnostic at item level beside ordinary `use` items.

use std::fmt::Display;

mod error;

/// A diagnostic for a build that lacks the feature the declaration needs.
#[cfg(not(feature = "std"))]
compile_error!("`std` is required for this fixture");

/// Keeps the import meaningful without a use site in this fixture.
fn describe(_: &dyn Display) {}