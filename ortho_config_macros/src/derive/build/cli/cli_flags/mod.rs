//! CLI flag validation and field builders for the derive macro.
//!
//! These helpers validate the short/long flags for each struct field and build
//! the generated CLI struct fields with the appropriate clap annotations.

use std::collections::HashSet;

use heck::ToKebabCase;
use quote::{quote, quote_spanned};
use syn::ext::IdentExt;
use syn::{Ident, Type};

use crate::derive::parse::{FieldAttrs, option_inner};

mod replay;

mod validation;
pub(super) use validation::{resolve_short_flag, validate_cli_long, validate_user_cli_short};

/// Generated CLI fields and the source-name reservations needed by later builders.
///
/// Keeping the emitted fields beside their reserved names lets shared flags
/// detect conflicts even when a source field was omitted from the CLI.
#[derive(Debug)]
pub(crate) struct CliStructTokens {
    /// Field declarations with the clap and serde attributes required by the derive.
    pub fields: Vec<proc_macro2::TokenStream>,
    /// Short options already claimed by emitted fields.
    pub used_shorts: HashSet<char>,
    /// Long options already claimed by emitted fields.
    pub used_longs: HashSet<String>,
    /// Every named source field, including fields omitted from the CLI, for reserved-name checks.
    pub field_names: HashSet<String>,
}

/// Resolved CLI spellings shared by generated help and documentation metadata.
#[derive(Debug, Clone)]
pub(crate) struct CliFieldMetadata {
    /// Original Rust field name used to associate the metadata with its source.
    pub field_name: String,
    /// Long option spelling after explicit overrides and defaults are applied.
    pub long: String,
    /// Short option assigned after duplicate and reserved-name checks.
    pub short: char,
    /// Whether the field uses optional boolean values in CLI parsing and metadata.
    pub is_bool: bool,
}

/// Wraps a field type in `Option` while avoiding a nested option for existing optional types.
pub(super) fn option_type_tokens(ty: &Type) -> proc_macro2::TokenStream {
    option_inner(ty).map_or_else(|| quote! { Option<#ty> }, |inner| quote! { Option<#inner> })
}

/// Whether a declared field type is `bool`, unwrapping a single `Option`.
///
/// Shared with the derive's CLI-info builder so optional boolean parsing and
/// its value-arity metadata use the same type classification. Independent
/// predicates could otherwise describe a different CLI from the one emitted.
pub(crate) fn is_bool_type(ty: &Type) -> bool {
    let inner = option_inner(ty).unwrap_or(ty);
    matches!(
        inner,
        Type::Path(type_path) if type_path.qself.is_none() && type_path.path.is_ident("bool")
    )
}

/// Reservations accumulated while source fields are resolved in declaration order.
struct CliFieldContext {
    /// Short options already assigned, preventing duplicate generated flags.
    used_shorts: HashSet<char>,
    /// Long options already assigned, including explicit user overrides.
    used_longs: HashSet<String>,
    /// Source field names reserved even when their fields opt out of CLI generation.
    field_names: HashSet<String>,
}

impl CliFieldContext {
    /// Creates empty reservations and preallocates field-name sets for the input size.
    fn with_capacity(capacity: usize) -> Self {
        Self {
            used_shorts: HashSet::new(),
            used_longs: HashSet::with_capacity(capacity),
            field_names: HashSet::with_capacity(capacity),
        }
    }
}

/// Resolved field identity and flag spellings before either output is generated.
struct ResolvedCliField {
    /// Source identifier retained for spans and generated Rust field names.
    name: Ident,
    /// Source spelling used to match metadata back to the declaration.
    field_name: String,
    /// Validated long option spelling.
    long: String,
    /// Validated and reserved short option.
    short: char,
    /// Selects optional boolean parsing and omits serde's `None` filter.
    is_bool: bool,
}

/// Resolves and validates CLI field information, updating context with used flags.
fn resolve_cli_field(
    field: &syn::Field,
    attrs: &FieldAttrs,
    context: &mut CliFieldContext,
) -> syn::Result<ResolvedCliField> {
    let Some(name) = field.ident.as_ref() else {
        return Err(syn::Error::new_spanned(
            field,
            "unnamed (tuple) fields are not supported for CLI derive",
        ));
    };

    let field_name = name.to_string();

    // `clap_derive` unraws the identifier *before* casing it, so `r#type`
    // yields the flag `--type`. `heck` splits words on any non-alphanumeric
    // character, so kebab-casing the raw spelling would give `--r-type` and
    // disagree with the flag clap renders for the same field.
    let long = attrs
        .cli_long
        .clone()
        .unwrap_or_else(|| name.unraw().to_string().to_kebab_case());
    validate_cli_long(name, &long)?;

    if !context.used_longs.insert(long.clone()) {
        return Err(syn::Error::new_spanned(
            name,
            format!("duplicate `cli_long` value '{long}'"),
        ));
    }

    let short = resolve_short_flag(name, attrs, &mut context.used_shorts)?;
    let is_bool = is_bool_type(&field.ty);

    Ok(ResolvedCliField {
        name: name.clone(),
        field_name,
        long,
        short,
        is_bool,
    })
}

/// Emits one public generated CLI field with clap parsing and serde omission behaviour.
fn process_cli_field(
    field: &syn::Field,
    attrs: &FieldAttrs,
    context: &mut CliFieldContext,
) -> syn::Result<proc_macro2::TokenStream> {
    let resolved = resolve_cli_field(field, attrs, context)?;

    let ty = option_type_tokens(&field.ty);
    let long_lit = syn::LitStr::new(&resolved.long, proc_macro2::Span::call_site());
    let short_lit = syn::LitChar::new(resolved.short, proc_macro2::Span::call_site());
    let span = resolved.name.span();
    let replay_attributes = replay::clap_replay_attributes(attrs, resolved.is_bool);

    let arg_attr = if resolved.is_bool {
        // Presence-only flags (`ArgAction::SetTrue`) can express `true` but not
        // an explicit `false`, so a `true` supplied by a file or environment
        // variable can never be cleared from the command line. Accepting an
        // optional `=<BOOL>` value keeps the bare flag meaning `true` while
        // making `--flag=false` reach the parser as a real `Some(false)`.
        //
        // `require_equals` keeps `--flag false` from reading the word after the
        // flag as its value, so a positional operand such as `--flag notes.txt`
        // still reaches the field it belongs to; `default_missing_value`
        // preserves the flag-only spelling. Absent flags stay `None` so lower
        // layers still win.
        //
        // `value_name` is set explicitly because clap otherwise derives the
        // placeholder from the field name, so `excited` would render as
        // `--excited[=<EXCITED>]`. Naming it `BOOL` matches the placeholder the
        // documentation IR reports, so generated help and the IR agree.
        quote_spanned! { span =>
            #[arg(
                long = #long_lit,
                short = #short_lit,
                #replay_attributes
                value_name = "BOOL",
                num_args = 0..=1,
                require_equals = true,
                default_missing_value = "true"
            )]
        }
    } else {
        quote_spanned! { span =>
            #[arg(long = #long_lit, short = #short_lit, #replay_attributes)]
        }
    };

    let serde_attr = if resolved.is_bool {
        proc_macro2::TokenStream::new()
    } else {
        quote_spanned! { span =>
            #[serde(skip_serializing_if = "Option::is_none")]
        }
    };

    let name = &resolved.name;
    Ok(quote_spanned! { span =>
        #arg_attr
        #serde_attr
        pub #name: #ty
    })
}

/// Captures the resolved field spellings used by generated documentation sections.
fn process_cli_metadata(
    field: &syn::Field,
    attrs: &FieldAttrs,
    context: &mut CliFieldContext,
) -> syn::Result<CliFieldMetadata> {
    let resolved = resolve_cli_field(field, attrs, context)?;

    Ok(CliFieldMetadata {
        field_name: resolved.field_name,
        long: resolved.long,
        short: resolved.short,
        is_bool: resolved.is_bool,
    })
}

/// Builds generated fields and returns reservations needed by later shared-flag generation.
///
/// `field_attrs` must align one-for-one with `fields`; mismatched slices are rejected to
/// prevent attributes from silently being applied to the wrong source field.
pub(crate) fn build_cli_struct_fields(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> syn::Result<CliStructTokens> {
    if fields.len() != field_attrs.len() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "CLI field metadata mismatch: expected {} `FieldAttrs` entries but found {}; lengths must match to avoid silently dropping metadata",
                fields.len(),
                field_attrs.len()
            ),
        ));
    }

    let mut context = CliFieldContext::with_capacity(fields.len());
    let mut result = Vec::with_capacity(fields.len());

    for (field, attrs) in fields.iter().zip(field_attrs) {
        if attrs.is_subcommand {
            continue;
        }
        // Register all field names (including skip_cli) so the config_path
        // reservation check in `build_config_flag_field` detects conflicts.
        if let Some(ident) = &field.ident {
            context.field_names.insert(ident.to_string());
        }
        if attrs.skip_cli {
            continue;
        }
        let field_tokens = process_cli_field(field, attrs, &mut context)?;
        result.push(field_tokens);
    }

    let CliFieldContext {
        used_shorts,
        used_longs,
        field_names,
    } = context;

    Ok(CliStructTokens {
        fields: result,
        used_shorts,
        used_longs,
        field_names,
    })
}

/// Resolves CLI names for fields without generating code, preserving field declaration order.
///
/// The returned entries omit subcommands and fields marked `skip_cli`, matching the
/// actual CLI surface used by the generated documentation.
pub(crate) fn build_cli_field_metadata(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> syn::Result<Vec<CliFieldMetadata>> {
    if fields.len() != field_attrs.len() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "CLI field metadata mismatch: expected {} `FieldAttrs` entries but found {}; lengths must match to avoid silently dropping metadata",
                fields.len(),
                field_attrs.len()
            ),
        ));
    }

    let mut context = CliFieldContext::with_capacity(fields.len());
    let mut result = Vec::with_capacity(fields.len());

    for (field, attrs) in fields.iter().zip(field_attrs) {
        if attrs.is_subcommand || attrs.skip_cli {
            continue;
        }
        result.push(process_cli_metadata(field, attrs, &mut context)?);
    }

    Ok(result)
}
