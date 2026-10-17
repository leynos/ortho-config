//! Field-level documentation IR generation.

mod defaults;
mod emission;
mod resolution;
mod shapes;
mod tokens;
mod validation;
mod value_types;

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;

use crate::derive::build::CliFieldMetadata;
use crate::derive::generate::localization::ArgIdsModel;
use crate::derive::generate::localization::suffix::MessageSuffix;
use crate::derive::parse::{FieldAttrs, SerdeRenameAll, serde_serialized_field_key};

use self::emission::render_field_metadata;
use self::resolution::{resolve_required, resolve_value_type};
pub(super) use self::shapes::FieldDocArgs;
use self::shapes::{
    FieldContext, FieldIdentity, FieldIdentityTokens, FieldMetadataComponents, IoTokens, MetaParts,
    ValueContext,
};
use super::AppName;
use super::sections::string_tokens;
use super::{example_tokens, link_tokens, note_tokens, option_char_tokens, option_string_tokens};
use defaults::{default_env_name, default_field_id};
use tokens::{build_possible_values, default_tokens, deprecated_tokens};
use validation::{ensure_unique, validate_env_name, validate_file_key};
use value_types::{is_multi_value, value_type_tokens};

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
        localization_args: args
            .localization_ids
            .args
            .iter()
            .map(|arg| (arg.field_name.as_str(), arg))
            .collect(),
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

/// Carries per-struct inputs and cross-field state across one field build.
///
/// The `*_seen` maps report a duplicate environment variable or file key at
/// the *second* declaration, which a per-field builder could not detect.
struct FieldMetaBuilder<'a> {
    /// Application name used to compose default catalogue identifiers.
    app_name: &'a AppName,
    /// Environment-variable prefix for this struct, when one is configured.
    prefix: Option<&'a str>,
    /// Container-level renaming applied when deriving a field's file key.
    serde_rename_all: Option<SerdeRenameAll>,
    /// CLI metadata collected for the struct, keyed by field name.
    cli_lookup: HashMap<&'a str, &'a CliFieldMetadata>,
    /// Localization argument ids for the struct, keyed by field name.
    localization_args: HashMap<&'a str, &'a ArgIdsModel>,
    /// Environment variables already claimed, with the span to blame.
    env_seen: HashMap<String, proc_macro2::Span>,
    /// File keys already claimed, with the span to blame.
    file_seen: HashMap<String, proc_macro2::Span>,
    /// Resolved crate path for generated code references.
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
        let help_id = self.identifier_tokens(
            attrs.doc.help_id.as_deref(),
            &field_name,
            MessageSuffix::Help,
        );
        let long_help_id = self.identifier_tokens(
            attrs.doc.long_help_id.as_deref(),
            &field_name,
            MessageSuffix::LongHelp,
        );
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
            help_id,
            long_help_id,
        };
        let components = FieldMetadataComponents {
            identity: identity.into_tokens(),
            value_context,
            io_tokens,
            meta_parts,
        };

        Ok(render_field_metadata(components, self.krate))
    }

    /// Renders the CLI, environment, and file metadata of one field.
    fn render_io_tokens(&mut self, context: &FieldContext<'_>) -> syn::Result<IoTokens> {
        Ok(IoTokens {
            cli: self.build_cli_tokens(context)?,
            env: self.build_env_tokens(context)?,
            file: self.build_file_tokens(context)?,
        })
    }

    /// Builds the expression for one field's catalogue identifier.
    ///
    /// An explicit id is emitted verbatim; otherwise a generated argument id
    /// defers to the runtime `message_id_for` composition, so the emitted
    /// constant agrees with the lookup that resolves it. Only a field with
    /// neither falls back to the default id composed here.
    fn identifier_tokens(
        &self,
        explicit: Option<&str>,
        field_name: &str,
        suffix: MessageSuffix,
    ) -> TokenStream {
        let segment: &str = suffix.as_ref();
        if let Some(identifier) = explicit {
            return string_tokens(identifier);
        }
        if let Some(arg) = self.localization_args.get(field_name) {
            let message_suffix = syn::LitStr::new(
                &format!("args.{}.{}", arg.name.as_ref(), segment),
                proc_macro2::Span::call_site(),
            );
            let krate = self.krate;
            return quote! { #krate::message_id_for(command_path, #message_suffix) };
        }
        string_tokens(&default_field_id(self.app_name, field_name, segment))
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
        let multiple = is_multi_value(&context.field.ty);
        let possible_values = build_possible_values(context.value_type);
        let hide_in_help = context.attrs.doc.cli_hide_in_help;

        // Boolean flags accept an optional `=<BOOL>` value. Report the value
        // placeholder and allowed literals so renderers can print the
        // `--flag[=<BOOL>]` form rather than an unexplained bare switch.
        // Every other option takes a required value, so only boolean flags
        // mark their value as optional.
        let (value_name, value_optional, resolved_possible_values) = if meta.is_bool {
            (
                option_string_tokens(Some("BOOL")),
                quote! { true },
                vec![
                    quote! { String::from("true") },
                    quote! { String::from("false") },
                ],
            )
        } else {
            (
                option_string_tokens(context.attrs.doc.cli_value_name.as_deref()),
                quote! { false },
                possible_values,
            )
        };

        let krate = self.krate;
        Ok(quote! {
            Some(#krate::docs::CliMetadata {
                long: #long,
                short: #short,
                value_name: #value_name,
                multiple: #multiple,
                takes_value: true,
                value_optional: #value_optional,
                possible_values: vec![ #( #resolved_possible_values ),* ],
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

/// Renders the default, deprecation, example, link, and note tokens of one field.
fn render_meta_parts(attrs: &FieldAttrs, krate: &proc_macro2::TokenStream) -> MetaParts {
    MetaParts {
        default_tokens: default_tokens(attrs, krate),
        deprecated_tokens: deprecated_tokens(attrs, krate),
        examples: example_tokens(&attrs.doc.examples, krate),
        links: link_tokens(&attrs.doc.links, krate),
        notes: note_tokens(&attrs.doc.notes, krate),
    }
}

impl FieldIdentity<'_> {
    /// Converts resolved source strings to literals at the macro call site.
    fn into_tokens(self) -> FieldIdentityTokens {
        FieldIdentityTokens {
            field_name: syn::LitStr::new(self.field_name, proc_macro2::Span::call_site()),
            help_id: self.help_id,
            long_help: self.long_help_id,
        }
    }
}
