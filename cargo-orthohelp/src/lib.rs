//! Library interface for `cargo-orthohelp` documentation tooling.
//!
//! This crate provides tools for generating documentation from `OrthoConfig`
//! intermediate representation (IR), including roff man pages and localised
//! JSON output, roff man pages, and `PowerShell` help artefacts.

pub mod error;
pub mod ir;
pub mod powershell;
pub mod roff;
pub mod schema;
