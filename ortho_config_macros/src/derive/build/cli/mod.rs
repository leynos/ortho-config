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

use std::collections::HashSet;

use syn::{Ident, Type};

pub(crate) use cli_flags::build_cli_struct_fields;
pub(crate) use cli_flags::{CliFieldMetadata, build_cli_field_metadata};

/// Emits an `Option<T>` type for CLI parsing while unwrapping one existing
/// `Option<T>` layer to avoid nested options.
pub(super) fn option_type_tokens(ty: &Type) -> proc_macro2::TokenStream {
    cli_flags::option_type_tokens(ty)
}

/// Checks a user-supplied long flag against reserved names and previously
/// claimed flags, returning a span-aware error for collisions.
pub(super) fn validate_cli_long(name: &Ident, long: &str) -> syn::Result<()> {
    cli_flags::validate_cli_long(name, long)
}

/// Validates an explicit short flag and returns it when it is available.
pub(super) fn validate_user_cli_short(
    name: &Ident,
    user: char,
    used_shorts: &HashSet<char>,
) -> syn::Result<char> {
    cli_flags::validate_user_cli_short(name, user, used_shorts)
}
