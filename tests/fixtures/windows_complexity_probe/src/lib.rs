//! Probe crate for the Windows-only cognitive-complexity gate.
//!
//! `make lint-whitaker-windows` scores the workspace's own Windows-only code.
//! This crate is the evidence that the command behind it still fails when it
//! should: a gate that silently stopped reporting would otherwise read as a
//! clean tree.
//!
//! Every item here is gated on `windows`, so a host-target run compiles the
//! crate and scores nothing — which is the same blind spot the workspace gate
//! exists to close. `in_limit` is the example that must pass. The
//! `over_limit` module, behind the Cargo feature of the same name, is the
//! example that must be reported.

#![deny(missing_docs)]

#[cfg(windows)]
mod in_limit;

#[cfg(windows)]
pub use in_limit::in_limit;

#[cfg(all(windows, feature = "over_limit"))]
pub mod over_limit;