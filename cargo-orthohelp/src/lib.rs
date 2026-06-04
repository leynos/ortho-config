//! Library interface for `cargo-orthohelp` documentation tooling.
//!
//! This crate provides tools for generating documentation from `OrthoConfig`
//! intermediate representation (IR), including localized JSON output, roff man
//! pages, and `PowerShell` help artefacts.

pub mod agent_context;
pub mod error;
pub mod ir;
pub mod policy;
pub mod powershell;
pub mod roff;
pub mod schema;
