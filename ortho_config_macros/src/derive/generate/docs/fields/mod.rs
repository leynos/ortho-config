//! Field-level documentation IR generation.

mod value_types;

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::Ident;

use crate::derive::build::CliFieldMetadata;
use crate::derive::parse::{
    FieldAttrs, SerdeRenameAll, btree_map_inner, hash_map_inner, option_inner, serde_has_default,
    serde_serialized_field_key, vec_inner,
};

use super::AppName;
use super::{example_tokens, link_tokens, note_tokens, option_char_tokens, option_string_tokens};
use value_types::{
    ValueTypeModel, enum_variants, infer_value_type, is_multi_value, parse_value_type_override,
    value_type_tokens,
};

pub(super) struct FieldDocArgs<'a> {
    pub app_name: &'a AppName,
    pub prefix: Option<&'a str>,
    pub fields: &'a [syn::Field],
    pub field_attrs: &'a [FieldAttrs],
    pub serde_rename_all: Option<SerdeRenameAll>,
    pub cli_fields: &'a [CliFieldMetadata],
}

pub(super) fn build_fields_metadata(args: &FieldDocArgs<'_>) -> syn::Result<Vec<TokenStream>> {
    if args.fields.len() != args.field_attrs.len() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "doc field metadata mismatch: expected {} FieldAttrs entries but found {}",
                args.fields.len(),
                args.field_attrs.len()
            ),
        ));
    }

    let cli_lookup = args
        .cli_fields
        .iter()
        .map(|meta| (meta.field_name.as_str(), meta))
        .collect::<HashMap<_, _>>();

    let mut builder = FieldMetaBuilder {
        app_name: args.app_name,
        prefix: args.prefix,
        serde_rename_all: args.serde_rename_all,
        cli_lookup,
        env_seen: HashMap::new(),
        file_seen: HashMap::new(),
    };

    let mut output = Vec::with_capacity(args.fields.len());
    for (field, attrs) in args.fields.iter().zip(args.field_attrs) {
        output.push(builder.build_field(field, attrs)?);
    }

    Ok(output)
}

struct FieldMetaBuilder<'a> {
    app_name: &'a AppName,
    prefix: Option<&'a str>,
    serde_rename_all: Option<SerdeRenameAll>,
    cli_lookup: HashMap<&'a str, &'a CliFieldMetadata>,
    env_seen: HashMap<String, proc_macro2::Span>,
    file_seen: HashMap<String, proc_macro2::Span>,
}

impl<'a> FieldMetaBuilder<'a> {
    #[expect(
        clippy::cognitive_complexity,
        reason = "`quote!` expansion inflates the complexity score; keep this linear and readable."
    )]
    fn build_field(
        &mut self,
        field: &'a syn::Field,
        attrs: &'a FieldAttrs,
    ) -> syn::Result<TokenStream> {
        let name = field
            .ident
            .as_ref()
            .ok_or_else(|| syn::Error::new_spanned(field, "tuple fields are not supported"))?;
        let field_name = name.to_string();
        let field_name_lit = syn::LitStr::new(&field_name, proc_macro2::Span::call_site());

        let help_id = attrs
            .doc
            .help_id
            .clone()
            .unwrap_or_else(|| default_field_id(self.app_name, &field_name, "help"));
        let long_help_id = attrs
            .doc
            .long_help_id
            .clone()
            .unwrap_or_else(|| default_field_id(self.app_name, &field_name, "long_help"));
        let value_type = resolve_value_type(attrs, field);
        let required = resolve_required(field, attrs)?;
        let value_tokens = value_type_tokens(value_type.clone());
        let context = FieldContext {
            name,
            field_name: &field_name,
            field,
            attrs,
            value_type: value_type.as_ref(),
        };

        let cli_tokens = self.build_cli_tokens(&context)?;
        let env_tokens = self.build_env_tokens(&context)?;
        let file_tokens = self.build_file_tokens(&context)?;

        let default_tokens = default_tokens(attrs);
        let deprecated_tokens = deprecated_tokens(attrs);
        let examples = example_tokens(&attrs.doc.examples);
        let links = link_tokens(&attrs.doc.links);
        let notes = note_tokens(&attrs.doc.notes);

        let help_id_lit = syn::LitStr::new(&help_id, proc_macro2::Span::call_site());
        let long_help_lit = syn::LitStr::new(&long_help_id, proc_macro2::Span::call_site());

        Ok(quote! {
            ortho_config::docs::FieldMetadata {
                name: String::from(#field_name_lit),
                help_id: String::from(#help_id_lit),
                long_help_id: Some(String::from(#long_help_lit)),
                value: #value_tokens,
                default: #default_tokens,
                required: #required,
                deprecated: #deprecated_tokens,
                cli: #cli_tokens,
                env: Some(#env_tokens),
                file: Some(#file_tokens),
                examples: vec![ #( #examples ),* ],
                links: vec![ #( #links ),* ],
                notes: vec![ #( #notes ),* ],
            }
        })
    }

    fn build_cli_tokens(&self, context: &FieldContext<'_>) -> syn::Result<TokenStream> {
        if context.attrs.skip_cli {
            return Ok(quote! { None });
        }
        let meta = self.cli_lookup.get(context.field_name).ok_or_else(|| {
            syn::Error::new_spanned(
                context.name,
                "missing CLI metadata for field; this is a macro bug",
            )
        })?;
        let long = option_string_tokens(Some(meta.long.as_str()));
        let short = option_char_tokens(Some(meta.short));
        let value_name = option_string_tokens(context.attrs.doc.cli_value_name.as_deref());
        let multiple = is_multi_value(&context.field.ty);
        let takes_value = !meta.is_bool;
        let possible_values = build_possible_values(context.value_type);
        let hide_in_help = context.attrs.doc.cli_hide_in_help;

        Ok(quote! {
            Some(ortho_config::docs::CliMetadata {
                long: #long,
                short: #short,
                value_name: #value_name,
                multiple: #multiple,
                takes_value: #takes_value,
                possible_values: vec![ #( #possible_values ),* ],
                hide_in_help: #hide_in_help,
            })
        })
    }

    fn build_env_tokens(&mut self, context: &FieldContext<'_>) -> syn::Result<TokenStream> {
        let env_name = context
            .attrs
            .doc
            .env_name
            .clone()
            .unwrap_or_else(|| default_env_name(self.prefix, context.field_name));
        validate_env_name(context.name, &env_name)?;
        ensure_unique("env", context.name, &env_name, &mut self.env_seen)?;
        let lit = syn::LitStr::new(&env_name, proc_macro2::Span::call_site());
        Ok(quote! {
            ortho_config::docs::EnvMetadata {
                var_name: String::from(#lit),
            }
        })
    }

    fn build_file_tokens(&mut self, context: &FieldContext<'_>) -> syn::Result<TokenStream> {
        let key_path = if let Some(key) = context.attrs.doc.file_key_path.clone() {
            key
        } else {
            serde_serialized_field_key(context.field, self.serde_rename_all)?
        };
        validate_file_key(context.name, &key_path)?;
        ensure_unique("file", context.name, &key_path, &mut self.file_seen)?;
        let lit = syn::LitStr::new(&key_path, proc_macro2::Span::call_site());
        Ok(quote! {
            ortho_config::docs::FileMetadata {
                key_path: String::from(#lit),
            }
        })
    }
}

struct FieldContext<'a> {
    name: &'a Ident,
    field_name: &'a str,
    field: &'a syn::Field,
    attrs: &'a FieldAttrs,
    value_type: Option<&'a ValueTypeModel>,
}

/// Helper to build optional metadata tokens with a single string field.
fn build_optional_doc_metadata(
    value: Option<&str>,
    struct_path: &TokenStream,
    field_name: &str,
) -> TokenStream {
    value.map_or_else(
        || quote! { None },
        |s| {
            let lit = syn::LitStr::new(s, proc_macro2::Span::call_site());
            let field_ident = syn::Ident::new(field_name, proc_macro2::Span::call_site());
            quote! {
                Some(#struct_path {
                    #field_ident: String::from(#lit),
                })
            }
        },
    )
}

fn default_tokens(attrs: &FieldAttrs) -> TokenStream {
    let display_str = attrs
        .default
        .as_ref()
        .map(|expr| expr.to_token_stream().to_string());

    build_optional_doc_metadata(
        display_str.as_deref(),
        &quote! { ortho_config::docs::DefaultValue },
        "display",
    )
}

fn deprecated_tokens(attrs: &FieldAttrs) -> TokenStream {
    build_optional_doc_metadata(
        attrs.doc.deprecated_note_id.as_deref(),
        &quote! { ortho_config::docs::Deprecation },
        "note_id",
    )
}

fn resolve_value_type(attrs: &FieldAttrs, field: &syn::Field) -> Option<ValueTypeModel> {
    attrs
        .doc
        .value_type
        .as_deref()
        .map(parse_value_type_override)
        .or_else(|| infer_value_type(&field.ty))
}

fn resolve_required(field: &syn::Field, attrs: &FieldAttrs) -> syn::Result<bool> {
    if let Some(required) = attrs.doc.required {
        return Ok(required);
    }
    if option_inner(&field.ty).is_some() {
        return Ok(false);
    }
    if attrs.default.is_some() {
        return Ok(false);
    }
    if serde_has_default(&field.attrs)? {
        return Ok(false);
    }
    // Collections (Vec, BTreeMap, HashMap) default to non-required since they can be empty.
    if is_collection_type(&field.ty) {
        return Ok(false);
    }
    Ok(true)
}

fn build_possible_values(value_type: Option<&ValueTypeModel>) -> Vec<TokenStream> {
    value_type
        .and_then(enum_variants)
        .map_or_else(Vec::new, |variants| {
            variants
                .iter()
                .map(|value| {
                    let lit = syn::LitStr::new(value, proc_macro2::Span::call_site());
                    quote! { String::from(#lit) }
                })
                .collect::<Vec<_>>()
        })
}

fn default_field_id(app_name: &AppName, field: &str, suffix: &str) -> String {
    format!("{}.fields.{field}.{suffix}", &**app_name)
}

fn default_env_name(prefix: Option<&str>, field: &str) -> String {
    let mut name = String::new();
    if let Some(prefix_value) = prefix {
        name.push_str(prefix_value);
    }
    name.push_str(field);
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn validate_env_name(field: &Ident, env_name: &str) -> syn::Result<()> {
    if env_name.is_empty() {
        return Err(syn::Error::new_spanned(
            field,
            "environment variable names must be non-empty",
        ));
    }
    if env_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        field,
        format!(
            "environment variable '{env_name}' must contain only ASCII alphanumeric characters or '_'",
        ),
    ))
}

fn validate_file_key(field: &Ident, key_path: &str) -> syn::Result<()> {
    if key_path.is_empty() {
        return Err(syn::Error::new_spanned(
            field,
            "file key paths must be non-empty",
        ));
    }
    for segment in key_path.split('.') {
        if segment.is_empty() {
            return Err(syn::Error::new_spanned(
                field,
                format!("file key path '{key_path}' must not contain empty segments"),
            ));
        }
        let valid = segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
        if !valid {
            return Err(syn::Error::new_spanned(
                field,
                format!(
                    "file key path '{key_path}' must contain only ASCII alphanumeric characters, '_' or '-'",
                ),
            ));
        }
    }
    Ok(())
}

fn ensure_unique(
    kind: &str,
    field: &Ident,
    key: &str,
    seen: &mut HashMap<String, proc_macro2::Span>,
) -> syn::Result<()> {
    if let Some(existing) = seen.get(key) {
        let mut err =
            syn::Error::new_spanned(field, format!("duplicate {kind} identifier '{key}'"));
        err.combine(syn::Error::new(*existing, "first defined here"));
        return Err(err);
    }
    seen.insert(key.to_owned(), field.span());
    Ok(())
}

/// Returns `true` if `ty` is a collection type (`Vec`, `BTreeMap`, `HashMap`).
fn is_collection_type(ty: &syn::Type) -> bool {
    vec_inner(ty).is_some() || btree_map_inner(ty).is_some() || hash_map_inner(ty).is_some()
}
