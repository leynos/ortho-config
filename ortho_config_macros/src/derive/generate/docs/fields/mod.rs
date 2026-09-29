//! Field-level documentation IR generation.

mod defaults;
mod render;
mod resolution;
mod tokens;
mod validation;
mod value_types;

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;

use crate::derive::build::CliFieldMetadata;
use crate::derive::parse::{FieldAttrs, SerdeRenameAll, serde_serialized_field_key};

use self::resolution::{resolve_required, resolve_value_type};
use super::AppName;
use super::{option_char_tokens, option_string_tokens};
use defaults::{default_env_name, default_field_id};
use render::{FieldContext, FieldIdentity, FieldMetadataComponents, IoTokens, ValueContext};
use render::{render_field_metadata, render_meta_parts};
use tokens::build_possible_values;
use validation::{ensure_unique, validate_env_name, validate_file_key};
use value_types::{is_multi_value, value_type_tokens};

/// Borrowed field and container inputs used to build localized field IR.
pub(super) struct FieldDocArgs<'a> {
    /// Application identity used when synthesizing default help identifiers.
    pub app_name: &'a AppName,
    /// Prefix prepended to generated environment-variable names, if configured.
    pub prefix: Option<&'a str>,
    /// Source fields whose generated documentation metadata must stay in order.
    pub fields: &'a [syn::Field],
    /// Parsed field options paired positionally with `fields`.
    pub field_attrs: &'a [FieldAttrs],
    /// Serde's struct-wide rename rule for file keys without an explicit override.
    pub serde_rename_all: Option<SerdeRenameAll>,
    /// CLI metadata indexed by source field name; skipped CLI fields need no entry.
    pub cli_fields: &'a [CliFieldMetadata],
    /// Resolved crate path for generated code references.
    pub krate: &'a proc_macro2::TokenStream,
}

/// Builds one generated documentation record per non-subcommand field.
///
/// The source fields and parsed attributes must have identical lengths because
/// they are paired by position. Duplicate environment names and file-key paths
/// are rejected while records are built so the generated metadata is
/// unambiguous.
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
        krate: args.krate,
    };

    let mut output = Vec::with_capacity(args.fields.len());
    for (field, attrs) in args.fields.iter().zip(args.field_attrs) {
        if attrs.is_subcommand {
            continue;
        }
        output.push(builder.build_field(field, attrs)?);
    }

    Ok(output)
}

/// Accumulates field records while checking names that must be unique per app.
struct FieldMetaBuilder<'a> {
    /// Application name used to derive default help identifiers.
    app_name: &'a AppName,
    /// Optional prefix used only when an environment name has no override.
    prefix: Option<&'a str>,
    /// Struct-level Serde rename rule used only when a file key has no override.
    serde_rename_all: Option<SerdeRenameAll>,
    /// CLI records keyed by source field name for constant-time field lookup.
    cli_lookup: HashMap<&'a str, &'a CliFieldMetadata>,
    /// First source span for each emitted environment name, for duplicate errors.
    env_seen: HashMap<String, proc_macro2::Span>,
    /// First source span for each emitted file key, for duplicate errors.
    file_seen: HashMap<String, proc_macro2::Span>,
    /// Resolved crate path used in every generated metadata type reference.
    krate: &'a proc_macro2::TokenStream,
}

impl<'a> FieldMetaBuilder<'a> {
    /// Resolves a field's identity and value type before assembling its metadata.
    ///
    /// Tuple fields are rejected because generated documentation identifies
    /// fields by name. CLI metadata must already exist for every field that is
    /// not marked `skip_cli`.
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
        let value_context = ValueContext {
            value_tokens: value_type_tokens(value_type.clone(), self.krate),
            required,
        };
        let context = FieldContext {
            name,
            field_name: &field_name,
            field,
            attrs,
            value_type: value_type.as_ref(),
        };

        let io_tokens = self.render_io_tokens(&context)?;
        let meta_parts = render_meta_parts(attrs, self.krate);
        let identity = FieldIdentity {
            field_name: &field_name,
            help_id: &help_id,
            long_help_id: &long_help_id,
        };
        let components = FieldMetadataComponents {
            identity: identity.into_tokens(),
            value_context,
            io_tokens,
            meta_parts,
        };

        Ok(render_field_metadata(components, self.krate))
    }

    /// Renders the CLI, environment, and file-source portions as one bundle.
    fn render_io_tokens(&mut self, context: &FieldContext<'_>) -> syn::Result<IoTokens> {
        Ok(IoTokens {
            cli: self.build_cli_tokens(context)?,
            env: self.build_env_tokens(context)?,
            file: self.build_file_tokens(context)?,
        })
    }

    /// Emits CLI metadata, preserving `None` for fields excluded from the CLI.
    ///
    /// The value name and help visibility come from documentation attributes;
    /// option spelling, multiplicity, and value-taking behaviour come from the
    /// already-resolved CLI metadata and Rust field type.
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

        let krate = self.krate;
        Ok(quote! {
            Some(#krate::docs::CliMetadata {
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

    /// Resolves and validates the field's environment name before tokenizing it.
    ///
    /// An explicit documentation override takes precedence over the generated
    /// prefix-and-field name. Duplicate names are errors, with spans retained
    /// so diagnostics can point back to the source fields.
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
        let krate = self.krate;
        Ok(quote! {
            #krate::docs::EnvMetadata {
                var_name: String::from(#lit),
            }
        })
    }

    /// Resolves and validates the file key path before tokenizing it.
    ///
    /// An explicit documentation key takes precedence over Serde's serialized
    /// field key, including the struct-level rename rule. Duplicate resolved
    /// paths are rejected to keep file-source metadata unambiguous.
    fn build_file_tokens(&mut self, context: &FieldContext<'_>) -> syn::Result<TokenStream> {
        let key_path = if let Some(key) = context.attrs.doc.file_key_path.clone() {
            key
        } else {
            serde_serialized_field_key(context.field, self.serde_rename_all)?
        };
        validate_file_key(context.name, &key_path)?;
        ensure_unique("file", context.name, &key_path, &mut self.file_seen)?;
        let lit = syn::LitStr::new(&key_path, proc_macro2::Span::call_site());
        let krate = self.krate;
        Ok(quote! {
            #krate::docs::FileMetadata {
                key_path: String::from(#lit),
            }
        })
    }
}
