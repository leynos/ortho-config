//! Parsing for clap argument declarations written on configuration fields.
//!
//! Documentation metadata must describe the command line the application
//! actually parses. When a field carries an explicit `#[arg(...)]` or
//! `#[clap(...)]` declaration, clap owns that field's CLI surface, so the
//! derive reads the declaration instead of synthesising a kebab-cased long
//! flag. Fields without a clap declaration keep the generated `OrthoConfig`
//! surface, which is the parser the configuration loader itself uses.
//!
//! Only syntax is inspected here; the model is resolved into documentation
//! metadata later, where declaration-order positional indices are available.

mod builder;
#[cfg(test)]
mod tests;
mod value_syntax;

use super::ClapRenameAll;
use super::clap_attrs::{clap_field_is_flattened, clap_field_is_subcommand, is_clap_attribute};
use builder::ClapArgBuilder;

/// One clap argument declaration as written on a source field.
#[expect(
    clippy::struct_excessive_bools,
    reason = "Each flag records an independent clap argument property."
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClapArgDecl {
    /// Resolved long flag, absent for a positional operand.
    pub(crate) long: Option<String>,
    /// Resolved short flag, absent when the declaration supplies none.
    pub(crate) short: Option<char>,
    /// Explicit or clap-derived value placeholder.
    pub(crate) value_name: String,
    /// Positional settings, present only when the field is an operand.
    pub(crate) positional: Option<ClapPositionalDecl>,
    /// Whether the argument consumes a value.
    pub(crate) takes_value: bool,
    /// Whether the value may be omitted, as in `--flag[=VALUE]`.
    pub(crate) value_optional: bool,
    /// Whether the argument accepts repeated values.
    pub(crate) multiple: bool,
    /// Whether clap hides the argument from help output.
    pub(crate) hide_in_help: bool,
    /// Resolved clap requiredness.
    pub(crate) required: bool,
    /// Whether clap skips the field entirely.
    pub(crate) is_skipped: bool,
}

/// Positional settings declared for one clap argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClapPositionalDecl {
    /// Explicit one-based index, absent when clap assigns one in field order.
    pub(crate) index: Option<u16>,
    /// Whether the operand accepts more than one value.
    pub(crate) variadic: bool,
    /// Whether the operand consumes trailing arguments without requiring `--`.
    pub(crate) var_arg: bool,
    /// Whether the operand is reachable only through `--`.
    pub(crate) last: bool,
}

/// Parses one field's clap argument declaration.
///
/// `rename_all` is the container's clap casing rule, applied to a bare `long`
/// or `short` that clap derives from the field name. Returns `None` when the
/// field declares no clap argument, or when it is a subcommand selector or
/// flattened container whose arguments this derive cannot map to individual
/// source fields.
pub(crate) fn parse_clap_arg_decl(
    field: &syn::Field,
    rename_all: ClapRenameAll,
) -> syn::Result<Option<ClapArgDecl>> {
    if clap_field_is_subcommand(field)? || clap_field_is_flattened(field)? {
        return Ok(None);
    }

    let mut builder = ClapArgBuilder::default();
    for attr in field.attrs.iter().filter(|attr| is_clap_attribute(attr)) {
        if !matches!(attr.meta, syn::Meta::List(_)) {
            continue;
        }
        attr.parse_nested_meta(|meta| builder.consume(&meta))?;
    }

    if builder.should_ignore() {
        return Ok(None);
    }
    builder.finish(field, rename_all).map(Some)
}
