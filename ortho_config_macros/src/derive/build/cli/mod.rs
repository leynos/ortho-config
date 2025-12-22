//! CLI helper builders used by the `OrthoConfig` derive macro.
//!
//! This module encapsulates generation and validation of CLI flag metadata.
//! It keeps track of claimed short and long flags to avoid collisions and
//! surfaces ergonomic diagnostics when callers need to supply overrides.

mod cli_flags;
#[cfg(test)]
mod tests;

pub(crate) use cli_flags::build_cli_struct_fields;
pub(crate) type CliStructTokens = cli_flags::CliStructTokens;
pub(super) use cli_flags::{option_type_tokens, validate_cli_long, validate_user_cli_short};
