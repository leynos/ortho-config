//! Field-level documentation IR generation.

mod defaults;
mod resolution;
mod tokens;
mod validation;
mod value_types;

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::derive::build::CliFieldMetadata;
use crate::derive::parse::{FieldAttrs, SerdeRenameAll, serde_serialized_field_key};

use self::resolution::{resolve_required, resolve_value_type};
use super::AppName;
use super::{example_tokens, link_tokens, note_tokens, option_char_tokens, option_string_tokens};
use defaults::{default_env_name, default_field_id};
use tokens::{build_possible_values, default_tokens, deprecated_tokens};
use validation::{ensure_unique, validate_env_name, validate_file_key};
use value_types::{ValueTypeModel, is_multi_value, value_type_tokens};

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

/// Source information shared by the independent field metadata renderers.
struct FieldContext<'a> {
    /// Identifier used to attach source spans to validation diagnostics.
    name: &'a Ident,
    /// Original Rust field spelling, also used to find its CLI record.
    field_name: &'a str,
    /// Full syntax node needed for type and Serde-key resolution.
    field: &'a syn::Field,
    /// Parsed per-field behaviour and documentation overrides.
    attrs: &'a FieldAttrs,
    /// Resolved type description, absent when neither override nor inference fits.
    value_type: Option<&'a ValueTypeModel>,
}

/// Source strings that identify a field in generated user-facing metadata.
struct FieldIdentity<'a> {
    /// Rust field name retained as the stable field identity.
    field_name: &'a str,
    /// Resolved short-help catalogue key.
    help_id: &'a str,
    /// Resolved long-help catalogue key.
    long_help_id: &'a str,
}

/// Value description and requiredness emitted together in a field record.
struct ValueContext {
    /// Tokens for `Some(ValueType)` or `None` when the type is not describable.
    value_tokens: TokenStream,
    /// Whether configuration must supply a value after defaults are considered.
    required: bool,
}

/// Generated token fragments for the field's supported input sources.
struct IoTokens {
    /// Optional CLI metadata; skipped fields emit `None`.
    cli: TokenStream,
    /// Environment-variable metadata, always present for a documented field.
    env: TokenStream,
    /// File key metadata, always present for a documented field.
    file: TokenStream,
}

/// Tokenized defaults and supplementary annotations attached to a field.
struct MetaParts {
    /// Default value metadata or `None` when no default is declared.
    default_tokens: TokenStream,
    /// Deprecation metadata or `None` when the field is current.
    deprecated_tokens: TokenStream,
    /// Ordered examples from the parsed documentation attributes.
    examples: Vec<TokenStream>,
    /// Ordered reference links from the parsed documentation attributes.
    links: Vec<TokenStream>,
    /// Ordered free-form notes from the parsed documentation attributes.
    notes: Vec<TokenStream>,
}

/// Converts parsed field annotations into tokens used in the metadata record.
fn render_meta_parts(attrs: &FieldAttrs, krate: &proc_macro2::TokenStream) -> MetaParts {
    MetaParts {
        default_tokens: default_tokens(attrs, krate),
        deprecated_tokens: deprecated_tokens(attrs, krate),
        examples: example_tokens(&attrs.doc.examples, krate),
        links: link_tokens(&attrs.doc.links, krate),
        notes: note_tokens(&attrs.doc.notes, krate),
    }
}

/// Complete intermediate record passed to the final metadata renderer.
struct FieldMetadataComponents {
    /// Pre-tokenized field name and help catalogue identifiers.
    identity: FieldIdentityTokens,
    /// Value type and requiredness rendered as a single metadata section.
    value_context: ValueContext,
    /// Input-source metadata for CLI, environment, and file configuration.
    io_tokens: IoTokens,
    /// Defaults and supplementary user-authored annotations.
    meta_parts: MetaParts,
}

/// Pre-computed string literals for identity fields in generated metadata.
struct FieldIdentityTokens {
    /// Literal preserving the source field's Rust spelling.
    field_name: syn::LitStr,
    /// Literal for the resolved short-help key.
    help_id: syn::LitStr,
    /// Literal for the resolved long-help key.
    long_help: syn::LitStr,
}

impl FieldIdentity<'_> {
    /// Converts resolved source strings to literals at the macro call site.
    fn into_tokens(self) -> FieldIdentityTokens {
        FieldIdentityTokens {
            field_name: syn::LitStr::new(self.field_name, proc_macro2::Span::call_site()),
            help_id: syn::LitStr::new(self.help_id, proc_macro2::Span::call_site()),
            long_help: syn::LitStr::new(self.long_help_id, proc_macro2::Span::call_site()),
        }
    }
}

/// Emits the public `FieldMetadata` initializer consumed by generated code.
fn render_field_metadata(
    components: FieldMetadataComponents,
    krate: &proc_macro2::TokenStream,
) -> TokenStream {
    let FieldMetadataComponents {
        identity,
        value_context,
        io_tokens,
        meta_parts,
    } = components;

    let identity_tokens = render_identity_tokens(identity);
    let value_tokens = render_value_tokens(value_context);
    let io = render_io_block(io_tokens);
    let meta = render_meta_block(meta_parts);

    quote! {
        #krate::docs::FieldMetadata {
            #identity_tokens
            #value_tokens
            #io
            #meta
        }
    }
}

/// Emits the field name and help identifiers with owned `String` values.
fn render_identity_tokens(identity: FieldIdentityTokens) -> TokenStream {
    let field_name = identity.field_name;
    let help_id = identity.help_id;
    let long_help = identity.long_help;
    quote! {
        name: String::from(#field_name),
        help_id: String::from(#help_id),
        long_help_id: Some(String::from(#long_help)),
    }
}

/// Emits the optional type description and requiredness flag.
fn render_value_tokens(value: ValueContext) -> TokenStream {
    let value_tokens = value.value_tokens;
    let required = value.required;
    quote! {
        value: #value_tokens,
        required: #required,
    }
}

/// Emits input-source metadata while preserving the optional CLI distinction.
fn render_io_block(io: IoTokens) -> TokenStream {
    let cli = io.cli;
    let env = io.env;
    let file = io.file;
    quote! {
        cli: #cli,
        env: Some(#env),
        file: Some(#file),
    }
}

/// Emits defaults, deprecation state, and all supplementary annotations.
fn render_meta_block(meta: MetaParts) -> TokenStream {
    let default = meta.default_tokens;
    let deprecated = meta.deprecated_tokens;
    let examples = render_vec_field("examples", &meta.examples);
    let links = render_vec_field("links", &meta.links);
    let notes = render_vec_field("notes", &meta.notes);
    quote! {
        default: #default,
        deprecated: #deprecated,
        #examples
        #links
        #notes
    }
}

/// Emits a named vector initializer while retaining the input item order.
fn render_vec_field(field_name: &str, items: &[TokenStream]) -> TokenStream {
    let ident = syn::Ident::new(field_name, proc_macro2::Span::call_site());
    quote! { #ident: vec![ #( #items ),* ], }
}
