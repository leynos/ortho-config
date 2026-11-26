//! Resolution of clap-declared CLI surface for documentation metadata.
//!
//! The parser records each field's declaration independently; positional
//! indices are a property of the declaration order of the whole struct, so
//! they are assigned here once every field is known. Explicit indices are
//! preserved, and clap's implicit counter advances only for operands that did
//! not name an index, matching `clap_builder`'s build step.

use std::collections::HashMap;

use crate::derive::parse::{ClapPositionalDecl, FieldAttrs};

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
    /// Explicit clap requiredness, when declared.
    pub(super) required: Option<bool>,
    /// Whether clap skips the field entirely.
    pub(super) is_skipped: bool,
}

/// Positional settings with the field's index resolved.
#[derive(Debug, Clone)]
pub(super) struct ResolvedPositional {
    /// One-based positional index.
    pub(super) index: u16,
    /// Whether the operand accepts more than one value.
    pub(super) variadic: bool,
    /// Whether the operand consumes trailing arguments without requiring `--`.
    pub(super) var_arg: bool,
    /// Whether the operand is reachable only through `--`.
    pub(super) last: bool,
}

/// Resolves every clap-declared field, assigning implicit positional indices.
///
/// Fields without a clap declaration are absent from the map, which keeps the
/// generated `OrthoConfig` surface for them.
pub(super) fn resolve_clap_args(
    fields: &[syn::Field],
    attrs: &[FieldAttrs],
) -> syn::Result<HashMap<String, ResolvedClapArg>> {
    let mut next_index: u16 = 1;
    let mut resolved = HashMap::new();

    for (field, field_attrs) in fields.iter().zip(attrs) {
        let Some(decl) = &field_attrs.clap_arg else {
            continue;
        };
        let Some(name) = field.ident.as_ref() else {
            continue;
        };

        let positional = if decl.is_skipped {
            None
        } else {
            decl.positional
                .as_ref()
                .map(|positional| resolve_positional(field, positional, &mut next_index))
                .transpose()?
        };

        resolved.insert(
            name.to_string(),
            ResolvedClapArg {
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
            },
        );
    }

    Ok(resolved)
}

/// Resolves one positional declaration, advancing the implicit index counter.
fn resolve_positional(
    field: &syn::Field,
    positional: &ClapPositionalDecl,
    next_index: &mut u16,
) -> syn::Result<ResolvedPositional> {
    let index = if let Some(index) = positional.index {
        index
    } else {
        let index = *next_index;
        *next_index = next_index.checked_add(1).ok_or_else(|| {
            syn::Error::new_spanned(
                field,
                "too many positional arguments; the documentation IR stores clap indices as u16",
            )
        })?;
        index
    };

    Ok(ResolvedPositional {
        index,
        variadic: positional.variadic,
        var_arg: positional.var_arg,
        last: positional.last,
    })
}

#[cfg(test)]
mod tests {
    //! Unit tests for positional index resolution.

    use super::*;

    /// Parses clap declarations for a field list.
    fn declarations(fields: &proc_macro2::TokenStream) -> (Vec<syn::Field>, Vec<FieldAttrs>) {
        let input: syn::DeriveInput = syn::parse2(quote::quote! {
            struct Fixture { #fields }
        })
        .expect("test fixture should parse");
        let syn::Data::Struct(data) = input.data else {
            panic!("test input must be a struct: {}", input.ident);
        };
        let parsed_fields: Vec<_> = data.fields.into_iter().collect();
        let attrs = parsed_fields
            .iter()
            .map(crate::derive::parse::parse_field_attrs)
            .collect::<syn::Result<Vec<_>>>()
            .expect("test field attributes should parse");
        (parsed_fields, attrs)
    }

    #[test]
    fn implicit_indices_follow_declaration_order() {
        let (fields, attrs) = declarations(&quote::quote! {
            #[arg(value_name = "SOURCE")]
            source: String,
            #[arg(long)]
            force: bool,
            #[arg(value_name = "DESTINATION")]
            destination: String
        });
        let resolved = resolve_clap_args(&fields, &attrs).expect("declarations should resolve");
        let source = resolved.get("source").expect("source resolves");
        let destination = resolved.get("destination").expect("destination resolves");
        assert_eq!(
            source.positional.as_ref().map(|value| value.index),
            Some(1),
            "the first implicit positional should receive index 1"
        );
        assert_eq!(
            destination.positional.as_ref().map(|value| value.index),
            Some(2),
            "the second implicit positional should receive index 2"
        );
        assert!(
            resolved
                .get("force")
                .is_some_and(|value| value.positional.is_none()),
            "a long flag must not receive a positional index"
        );
    }

    #[test]
    fn explicit_indices_do_not_advance_the_implicit_counter() {
        let (fields, attrs) = declarations(&quote::quote! {
            #[arg(value_name = "FIRST")]
            first: String,
            #[arg(index = 5, value_name = "FIFTH")]
            fifth: String,
            #[arg(value_name = "SECOND")]
            second: String
        });
        let resolved = resolve_clap_args(&fields, &attrs).expect("declarations should resolve");
        assert_eq!(
            resolved
                .get("first")
                .and_then(|value| value.positional.as_ref())
                .map(|value| value.index),
            Some(1),
            "the first implicit positional should receive index 1"
        );
        assert_eq!(
            resolved
                .get("fifth")
                .and_then(|value| value.positional.as_ref())
                .map(|value| value.index),
            Some(5),
            "the explicit positional index should be preserved"
        );
        assert_eq!(
            resolved
                .get("second")
                .and_then(|value| value.positional.as_ref())
                .map(|value| value.index),
            Some(2),
            "an explicit index must not advance the implicit counter"
        );
    }

    #[test]
    fn skipped_fields_receive_no_positional_index() {
        let (fields, attrs) = declarations(&quote::quote! {
            #[arg(skip)]
            hidden: String,
            #[arg(value_name = "INPUT")]
            input: String
        });
        let resolved = resolve_clap_args(&fields, &attrs).expect("declarations should resolve");
        assert!(
            resolved.get("hidden").is_some_and(|value| value.is_skipped),
            "an arg(skip) field should be marked skipped"
        );
        assert_eq!(
            resolved
                .get("input")
                .and_then(|value| value.positional.as_ref())
                .map(|value| value.index),
            Some(1),
            "a skipped field must not consume the first positional index"
        );
    }
}
