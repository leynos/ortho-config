//! CLI flag validation and field builders for the derive macro.
//!
//! These helpers validate the short/long flags for each struct field and build
//! the generated CLI struct fields with the appropriate clap annotations.

use std::collections::HashSet;

use heck::ToKebabCase;
use quote::{quote, quote_spanned};
use syn::{Ident, Type};

use crate::derive::parse::{ClapInferredDefault, FieldAttrs, option_inner};

mod validation;
pub(super) use validation::{resolve_short_flag, validate_cli_long, validate_user_cli_short};

/// Generated CLI fields and the reservations needed when adding shared flags.
#[derive(Debug)]
pub(crate) struct CliStructTokens {
    /// Field declarations with their clap and serde attributes.
    pub fields: Vec<proc_macro2::TokenStream>,
    /// Short flags claimed by fields, used to avoid collisions with injected flags.
    pub used_shorts: HashSet<char>,
    /// Long flags claimed by fields, used to avoid collisions with injected flags.
    pub used_longs: HashSet<String>,
    /// Every named source field, including fields omitted from the CLI, for reserved-name checks.
    pub field_names: HashSet<String>,
}

/// Resolved flag details shared with documentation and downstream derive generation.
#[derive(Debug, Clone)]
pub(crate) struct CliFieldMetadata {
    /// Rust field name used to associate generated CLI arguments with config fields.
    pub field_name: String,
    /// Validated long spelling consumed by clap and shown in generated help.
    pub long: String,
    /// Validated short spelling consumed by clap and shown in generated help.
    pub short: char,
    /// Whether the field accepts boolean switch semantics rather than a value.
    pub is_bool: bool,
}

/// Wraps a field type in `Option` while avoiding a nested option for existing optional types.
pub(super) fn option_type_tokens(ty: &Type) -> proc_macro2::TokenStream {
    option_inner(ty).map_or_else(|| quote! { Option<#ty> }, |inner| quote! { Option<#inner> })
}

/// Recognizes plain and optional `bool` fields for clap's switch-style argument handling.
fn is_bool_type(ty: &Type) -> bool {
    let inner = option_inner(ty).unwrap_or(ty);
    matches!(
        inner,
        Type::Path(type_path) if type_path.qself.is_none() && type_path.path.is_ident("bool")
    )
}

/// Replay parser settings that affect values accepted by the generated CLI.
fn clap_replay_attributes(attrs: &FieldAttrs, is_bool: bool) -> proc_macro2::TokenStream {
    let Some(ClapInferredDefault::Value(default)) = attrs.inferred_clap_default.as_ref() else {
        return proc_macro2::TokenStream::new();
    };

    let bool_default = is_bool.then(|| {
        let value = &default.value;
        quote! { default_value = #value, }
    });
    let value_parser = default.value_parser.as_ref().map(|parser| {
        quote! { value_parser = #parser, }
    });
    let value_enum = default.value_enum.then(|| quote! { value_enum, });
    let value_delimiter = default.value_delimiter.as_ref().map(|delimiter| {
        quote! { value_delimiter = #delimiter, }
    });
    let ignore_case = default.ignore_case.as_ref().map(|ignore_case| {
        quote! { ignore_case = #ignore_case, }
    });

    quote! {
        #bool_default
        #value_parser
        #value_enum
        #value_delimiter
        #ignore_case
    }
}

/// Context for tracking used CLI flags and field names during field processing.
struct CliFieldContext {
    /// Short spellings claimed by fields processed so far.
    used_shorts: HashSet<char>,
    /// Long spellings claimed by fields processed so far.
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

/// Resolved CLI field information shared by both struct field and metadata generation.
struct ResolvedCliField {
    /// Original identifier retained to preserve source spans in generated diagnostics and tokens.
    name: Ident,
    /// Rust spelling used to join this field to generated documentation metadata.
    field_name: String,
    /// Validated long spelling emitted in the clap attribute.
    long: String,
    /// Unique short spelling emitted in the clap attribute.
    short: char,
    /// Determines whether generation uses a switch action and omits serde's `None` filter.
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

    let long = attrs
        .cli_long
        .clone()
        .unwrap_or_else(|| field_name.to_kebab_case());
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

/// Emits one public generated CLI field with clap parsing and serde omission behavior.
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
    let replay_attributes = clap_replay_attributes(attrs, resolved.is_bool);

    let arg_attr = if resolved.is_bool {
        quote_spanned! { span =>
            #[arg(
                long = #long_lit,
                short = #short_lit,
                #replay_attributes
                action = clap::ArgAction::SetTrue
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
