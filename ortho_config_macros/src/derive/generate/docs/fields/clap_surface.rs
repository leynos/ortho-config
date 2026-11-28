//! Resolution of clap-declared CLI surface for documentation metadata.
//!
//! The parser records each field's declaration independently; positional
//! indices are a property of the declaration order of the whole struct, so
//! they are assigned here once every field is known. Explicit indices are
//! preserved, and clap's implicit counter advances only for operands that did
//! not name an index, matching `clap_builder`'s build step.

use std::collections::HashMap;

use syn::ext::IdentExt;

use crate::derive::parse::{
    ClapArgDecl, ClapPositionalDecl, FieldAttrs, clap_field_is_flattened, clap_field_is_subcommand,
    inferred_argument_shape, is_clap_attribute, value_name,
};

/// One field's resolved clap CLI surface.
#[expect(
    clippy::struct_excessive_bools,
    reason = "Each flag records an independent clap argument property."
)]
#[derive(Debug, Clone)]
pub(super) struct ResolvedClapArg {
    /// Resolved long flag, absent for a positional operand.
    pub(super) long: Option<String>,
    /// Resolved short flag, absent when the declaration supplies none.
    pub(super) short: Option<char>,
    /// Explicit or clap-derived value placeholder.
    pub(super) value_name: String,
    /// Positional settings, present only for an operand.
    pub(super) positional: Option<ResolvedPositional>,
    /// Whether the argument consumes a value.
    pub(super) takes_value: bool,
    /// Whether the value may be omitted.
    pub(super) value_optional: bool,
    /// Whether the argument accepts repeated values.
    pub(super) multiple: bool,
    /// Whether clap hides the argument from help output.
    pub(super) hide_in_help: bool,
    /// Resolved clap requiredness.
    pub(super) required: bool,
    /// Whether clap skips the field entirely.
    pub(super) is_skipped: bool,
    /// Whether the argument must not be published because its absolute
    /// positional index is unknown.
    pub(super) is_unpublished: bool,
}

/// Positional settings with the field's index resolved.
#[expect(
    clippy::struct_excessive_bools,
    reason = "Each flag records an independent clap positional property."
)]
#[derive(Debug, Clone)]
pub(super) struct ResolvedPositional {
    /// One-based positional index.
    pub(super) index: u16,
    /// Whether clap assigned the index implicitly rather than from an
    /// `index` key.
    pub(super) implicit: bool,
    /// Whether the operand accepts more than one value.
    pub(super) variadic: bool,
    /// Whether the operand consumes trailing arguments without requiring `--`.
    pub(super) var_arg: bool,
    /// Whether the operand is reachable only through `--`.
    pub(super) last: bool,
}

/// Resolves every clap argument the source struct describes.
///
/// A field with a readable declaration is described from it. When any field in
/// the struct declares a clap argument, the struct necessarily derives a clap
/// trait, so a field with no declaration is a clap-derived argument as well:
/// it is a positional operand unless its type is a bare `bool`, which clap
/// rejects as a positional switch. A struct with no declaration at all is
/// indistinguishable from a configuration-only type, so its fields stay absent
/// from the map and keep the generated `OrthoConfig` surface.
pub(super) fn resolve_clap_args(
    fields: &[syn::Field],
    attrs: &[FieldAttrs],
) -> syn::Result<HashMap<String, ResolvedClapArg>> {
    let mut next_index: u16 = 1;
    let mut resolved = HashMap::new();
    let declares_arguments = declares_clap_arguments(fields)?;
    // A flattened container contributes operands the derive cannot count, so
    // an implicit index assigned after it would be a guess.
    let mut after_flatten = false;

    for (field, field_attrs) in fields.iter().zip(attrs) {
        let Some(name) = field.ident.as_ref() else {
            continue;
        };
        if clap_field_is_flattened(field)? {
            after_flatten = true;
            continue;
        }
        if let Some(decl) = &field_attrs.clap_arg {
            if decl.is_unreadable {
                advance_unreadable_operand(field, decl, &mut next_index)?;
                continue;
            }
            let argument = resolve_declared_argument(field, decl, &mut next_index)?;
            resolved.insert(
                name.to_string(),
                suppress_if_index_unknown(argument, after_flatten),
            );
            continue;
        }
        if let Some(argument) =
            infer_if_declared_argument(field, name, declares_arguments, &mut next_index)?
        {
            resolved.insert(
                name.to_string(),
                suppress_if_index_unknown(argument, after_flatten),
            );
        }
    }

    Ok(resolved)
}

/// Marks an argument unpublished when its absolute index is unknown.
///
/// A flattened container's own operands are invisible to the parent derive, so
/// any later implicit index may be short by the child's operand count. Named
/// options and explicitly indexed operands keep their documented surface.
fn suppress_if_index_unknown(
    mut argument: ResolvedClapArg,
    after_flatten: bool,
) -> ResolvedClapArg {
    let index_unknown = after_flatten
        && argument
            .positional
            .as_ref()
            .is_some_and(|positional| positional.implicit);
    argument.is_unpublished = index_unknown;
    argument
}

/// Resolves one field from its readable clap declaration.
fn resolve_declared_argument(
    field: &syn::Field,
    decl: &ClapArgDecl,
    next_index: &mut u16,
) -> syn::Result<ResolvedClapArg> {
    let positional = if decl.is_skipped {
        None
    } else {
        decl.positional
            .as_ref()
            .map(|positional| resolve_positional(field, positional, next_index))
            .transpose()?
    };
    Ok(ResolvedClapArg {
        long: decl.long.clone(),
        short: decl.short,
        value_name: decl.value_name.clone(),
        positional,
        takes_value: decl.takes_value,
        value_optional: decl.value_optional,
        multiple: decl.multiple,
        hide_in_help: decl.hide_in_help,
        required: decl.required,
        is_skipped: decl.is_skipped,
        is_unpublished: false,
    })
}

/// Advances the implicit index counter for an unreadable operand.
///
/// The field's CLI metadata is omitted because part of its declaration could
/// not be read, but clap still assigns it the next implicit index when it names
/// no index of its own. Later declared operands must count it, or their
/// documented index drifts from the index clap parses.
fn advance_unreadable_operand(
    field: &syn::Field,
    decl: &ClapArgDecl,
    next_index: &mut u16,
) -> syn::Result<()> {
    let implicit_operand =
        !decl.is_skipped && decl.positional.is_some() && !decl.has_explicit_index;
    if implicit_operand {
        advance_index(field, next_index)?;
    }
    Ok(())
}

/// Infers an argument only for an undeclared field of a clap command.
///
/// A declaration the derive cannot read is left out: synthesising a positional
/// might contradict a `long` the unreadable tokens hide.
fn infer_if_declared_argument(
    field: &syn::Field,
    name: &syn::Ident,
    declares_arguments: bool,
    next_index: &mut u16,
) -> syn::Result<Option<ResolvedClapArg>> {
    if !declares_arguments || field.attrs.iter().any(is_clap_attribute) {
        return Ok(None);
    }
    infer_undeclared_argument(field, name, next_index)
}

/// Reports whether the struct is a clap command with fields of its own.
///
/// A clap helper attribute only compiles when a clap derive registered it, so
/// any one of them proves the struct is a clap command: an argument
/// declaration, a flattened container whose arguments join this command's
/// list, or a subcommand selector. The selector itself owns no argument and is
/// excluded from the field metadata and the positional counter.
fn declares_clap_arguments(fields: &[syn::Field]) -> syn::Result<bool> {
    for field in fields {
        if field_declares_clap_argument(field)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Reports whether one field proves the struct is a clap command.
fn field_declares_clap_argument(field: &syn::Field) -> syn::Result<bool> {
    let declares = field.attrs.iter().any(is_clap_attribute);
    Ok(declares || clap_field_is_flattened(field)? || clap_field_is_subcommand(field)?)
}

/// Describes a field that clap derives an argument for without a declaration.
///
/// The field is a positional operand: clap creates no long or short flag for a
/// field that names neither. A bare `bool` is left to the generated surface
/// because clap derives `SetTrue` for it and then rejects a value-less
/// positional switch, so such a struct cannot be a valid clap command.
fn infer_undeclared_argument(
    field: &syn::Field,
    name: &syn::Ident,
    next_index: &mut u16,
) -> syn::Result<Option<ResolvedClapArg>> {
    if clap_field_is_subcommand(field)? || clap_field_is_flattened(field)? {
        return Ok(None);
    }
    let shape = inferred_argument_shape(&field.ty);
    if shape.is_bool {
        return Ok(None);
    }
    let raw_field = name.unraw().to_string();
    let declared = ClapPositionalDecl {
        index: None,
        variadic: shape.is_repeated,
        var_arg: false,
        last: false,
    };
    Ok(Some(ResolvedClapArg {
        long: None,
        short: None,
        value_name: value_name(None, None, &raw_field),
        positional: Some(resolve_positional(field, &declared, next_index)?),
        takes_value: true,
        value_optional: shape.is_value_optional,
        multiple: shape.is_repeated,
        hide_in_help: false,
        required: !shape.is_optional,
        is_skipped: false,
        is_unpublished: false,
    }))
}

/// Resolves one positional declaration, advancing the implicit index counter.
fn resolve_positional(
    field: &syn::Field,
    positional: &ClapPositionalDecl,
    next_index: &mut u16,
) -> syn::Result<ResolvedPositional> {
    let index = match positional.index {
        Some(index) => index,
        None => advance_index(field, next_index)?,
    };

    Ok(ResolvedPositional {
        index,
        implicit: positional.index.is_none(),
        variadic: positional.variadic,
        var_arg: positional.var_arg,
        last: positional.last,
    })
}

/// Assigns clap's next implicit positional index.
fn advance_index(field: &syn::Field, next_index: &mut u16) -> syn::Result<u16> {
    let index = *next_index;
    *next_index = next_index.checked_add(1).ok_or_else(|| {
        syn::Error::new_spanned(
            field,
            "too many positional arguments; the documentation IR stores clap indices as u16",
        )
    })?;
    Ok(index)
}

#[cfg(test)]
#[path = "clap_surface_tests.rs"]
mod tests;
