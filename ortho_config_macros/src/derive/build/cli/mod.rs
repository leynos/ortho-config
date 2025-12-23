//! CLI helper builders used by the `OrthoConfig` derive macro.
//!
//! This module encapsulates generation and validation of CLI flag metadata.
//! It keeps track of claimed short and long flags to avoid collisions and
//! surfaces ergonomic diagnostics when callers need to supply overrides.
//!
//! Use the re-exports from this module when working in the derive builder; the
//! `cli_flags` submodule is an internal detail and should not be depended on
//! directly to avoid coupling to the private layout.

mod cli_flags;
#[cfg(test)]
mod tests;

pub(crate) use cli_flags::build_cli_struct_fields;
pub(super) use cli_flags::{option_type_tokens, validate_cli_long, validate_user_cli_short};
