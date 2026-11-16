//! Required-value schema emission for `#[derive(OrthoConfig)]`.
//!
//! The generated implementation records only what the derive already knows:
//! field deserialization keys, aliases, and the requiredness the attribute
//! grammar decides. It never runs at macro time against user data, and it never
//! consults the environment, the process arguments, or a configuration file.

use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::{Field, Ident};

use self::requiredness::{ActivationFacts, FieldActivation, activation};
use crate::derive::build::CliFieldMetadata;
use crate::derive::parse::{
    FieldAttrs, SerdeRenameAll, StructAttrs, nested_schema_type, option_inner,
    serde_container_is_transparent, serde_deserialized_field_key, serde_field_aliases,
    serde_field_has_custom_deserializer, serde_field_is_flattened,
    serde_field_skips_deserialization, serde_has_default, serde_rename_rules,
};

mod requiredness;

/// Parsed inputs needed to emit one required-value schema implementation.
pub(crate) struct RequiredSchemaArgs<'a> {
    /// Configuration struct receiving the implementation.
    pub ident: &'a Ident,
    /// Generic parameters and bounds declared by the struct.
    pub generics: &'a syn::Generics,
    /// Declared fields in source order.
    pub fields: &'a [Field],
    /// Parsed `#[ortho_config(...)]` field attributes, parallel to `fields`.
    pub field_attrs: &'a [FieldAttrs],
    /// Parsed struct-level attributes, used for the environment prefix.
    pub struct_attrs: &'a StructAttrs,
    /// Container-level naming rule applied when deserializing.
    pub deserialize_rename_all: Option<SerdeRenameAll>,
    /// Whether the container carries `#[serde(default)]`, making every field
    /// optional at the deserializer.
    pub container_default: bool,
    /// Whether the container is `#[serde(transparent)]`, so its fields describe
    /// the inner value rather than the object a parent key holds.
    pub container_transparent: bool,
    /// Whether this derive also emits the loader that materializes declared
    /// defaults, which only [`crate::derive::OrthoConfig`] does.
    pub materializes_defaults: bool,
    /// Resolved CLI flags, used only for fields whose key matches the parser.
    pub cli_metadata: &'a [CliFieldMetadata],
    /// Resolved crate path for downstream macro expansion.
    pub krate: &'a TokenStream,
}

/// Resolved macro inputs for one required-value schema expansion.
///
/// Both derive entry points build this shape, so they cannot disagree about
/// which container attributes contribute naming rules or defaults.
pub(crate) struct RequiredSchemaSource<'a> {
    /// Configuration struct receiving the implementation.
    pub ident: &'a Ident,
    /// Generic parameters and bounds declared by the struct.
    pub generics: &'a syn::Generics,
    /// Declared fields in source order.
    pub fields: &'a [Field],
    /// Parsed `#[ortho_config(...)]` field attributes, parallel to `fields`.
    pub field_attrs: &'a [FieldAttrs],
    /// Parsed struct-level attributes, used for the environment prefix.
    pub struct_attrs: &'a StructAttrs,
    /// Container attributes, read for Serde naming and default rules.
    pub container_attrs: &'a [syn::Attribute],
    /// Resolved CLI flags, used only for fields whose key matches the parser.
    pub cli_metadata: &'a [CliFieldMetadata],
    /// Whether the companion derive also emits the loader that materializes
    /// declared defaults.
    pub materializes_defaults: bool,
    /// Resolved crate path for downstream macro expansion.
    pub krate: &'a TokenStream,
}

impl RequiredSchemaSource<'_> {
    /// Resolve container-level Serde rules and emit the implementation.
    ///
    /// # Errors
    ///
    /// Returns a syntax error when a field cannot produce a deserialization key
    /// or its Serde attributes cannot be parsed.
    pub(crate) fn generate(&self) -> syn::Result<TokenStream> {
        let rules = serde_rename_rules(self.container_attrs)?;
        generate_required_schema_impl(&RequiredSchemaArgs {
            ident: self.ident,
            generics: self.generics,
            fields: self.fields,
            field_attrs: self.field_attrs,
            struct_attrs: self.struct_attrs,
            deserialize_rename_all: rules.deserialize,
            container_default: serde_has_default(self.container_attrs)?,
            container_transparent: serde_container_is_transparent(self.container_attrs)?,
            materializes_defaults: self.materializes_defaults,
            cli_metadata: self.cli_metadata,
            krate: self.krate,
        })
    }
}

/// Emit `impl RequiredConfigSchema` for the configuration struct.
///
/// # Errors
///
/// Returns a syntax error when a field cannot produce a deserialization key or
/// its Serde attributes cannot be parsed.
pub(crate) fn generate_required_schema_impl(
    args: &RequiredSchemaArgs<'_>,
) -> syn::Result<TokenStream> {
    let statements = if args.container_transparent {
        // A transparent container reads its inner value from the parent's key,
        // so nothing here is a key of the containing object.
        Vec::new()
    } else {
        field_statements(args)?
    };
    let hints = cli_hints(args)?;
    let prefix = prefix_tokens(args.struct_attrs);
    let ident = args.ident;
    let krate = args.krate;
    // A generic child keeps its declared bounds, so `T: RequiredConfigSchema`
    // stays the caller's explicit contract rather than an inferred one.
    let (impl_generics, ty_generics, where_clause) = args.generics.split_for_impl();
    let fields_decl = if statements.is_empty() {
        quote! {
            let fields: ::std::vec::Vec<#krate::SchemaField> = ::std::vec::Vec::new();
        }
    } else {
        quote! {
            let mut fields: ::std::vec::Vec<#krate::SchemaField> = ::std::vec::Vec::new();
        }
    };

    let declares_nested_object = args.container_transparent.then(|| {
        quote! {
            fn declares_nested_object() -> bool {
                false
            }
        }
    });

    Ok(quote! {
        impl #impl_generics #krate::RequiredConfigSchema for #ident #ty_generics
        #where_clause
        {
            fn collect_required_schema(
                stack: &mut #krate::RequiredSchemaStack,
            ) -> ::std::result::Result<
                ::std::vec::Vec<#krate::SchemaField>,
                #krate::RequiredSchemaError,
            > {
                stack.enter::<Self>()?;
                #fields_decl
                #( #statements )*
                stack.leave::<Self>();
                ::std::result::Result::Ok(fields)
            }

            #declares_nested_object

            fn required_route_hints() -> #krate::RouteHints {
                #krate::RouteHints {
                    prefix: #prefix,
                    cli_flags: ::std::vec![ #( #hints ),* ],
                }
            }
        }
    })
}

/// Build one statement per field that participates in the schema.
fn field_statements(args: &RequiredSchemaArgs<'_>) -> syn::Result<Vec<TokenStream>> {
    let mut statements = Vec::new();
    for (field, attrs) in args.fields.iter().zip(args.field_attrs) {
        if !participates(field, attrs)? {
            continue;
        }
        let key = serde_deserialized_field_key(field, args.deserialize_rename_all)?;
        if serde_field_is_flattened(&field.attrs)? {
            statements.push(flatten_statement(args, field));
            continue;
        }
        statements.push(push_statement(args, field, attrs, &key)?);
    }
    Ok(statements)
}

/// Build the statement that appends one flattened composition.
///
/// A flattened field has no key of its own, so no default can be aimed at one
/// at this level, and Serde only resolves the child to absence when the field
/// is an `Option`. A defaulted flattened struct is still deserialized from the
/// unmatched keys, so a missing child leaf propagates instead of falling back
/// to `Default`; its requirements therefore stay required here. Composing an
/// `Option` child unchanged would invent an error the loader never raises, so
/// its whole composed subtree becomes optional — including the descendants of
/// any nested object it contains, because a single failing leaf is what makes
/// Serde fall back to `None`.
fn flatten_statement(args: &RequiredSchemaArgs<'_>, field: &Field) -> TokenStream {
    let krate = args.krate;
    let ty = nested_schema_type(&field.ty);
    // A transparent child composes nothing: flattening one forwards to an
    // inner value whose keys this schema does not record, so composing would
    // name keys the loader never reads.
    let composed = quote! {
        #krate::SchemaChildren::compose::<#ty>(stack)?
    };
    if option_inner(&field.ty).is_some() {
        return quote! {
            if let #krate::SchemaChildren::Nested(children) = #composed {
                fields.extend(children.into_iter().map(#krate::SchemaField::optional_subtree));
            }
        };
    }
    quote! {
        if let #krate::SchemaChildren::Nested(children) = #composed {
            fields.extend(children);
        }
    }
}

/// Build the statement that appends one leaf or nested requirement.
fn push_statement(
    args: &RequiredSchemaArgs<'_>,
    field: &Field,
    attrs: &FieldAttrs,
    key: &str,
) -> syn::Result<TokenStream> {
    let krate = args.krate;
    let aliases = serde_field_aliases(&field.attrs)?;
    let classification = activation(
        field,
        attrs,
        &ActivationFacts {
            container_default: args.container_default,
            materializes_defaults: args.materializes_defaults,
            key,
            aliases: &aliases,
        },
    )?;
    let activation = match classification {
        FieldActivation::Required => quote! { #krate::Activation::Required },
        FieldActivation::Optional => quote! { #krate::Activation::Optional },
        FieldActivation::DefaultBacked => quote! { #krate::Activation::DefaultBacked },
    };
    let children = if composes_children(field, attrs)? {
        let ty = nested_schema_type(&field.ty);
        quote! { #krate::SchemaChildren::compose::<#ty>(stack)? }
    } else {
        quote! { #krate::SchemaChildren::Leaf }
    };

    Ok(quote! {
        fields.push(#krate::SchemaField {
            key: ::std::string::String::from(#key),
            aliases: ::std::vec![
                #( ::std::string::String::from(#aliases) ),*
            ],
            activation: #activation,
            children: #children,
        });
    })
}

/// Decide whether a field can appear in the schema at all.
///
/// A subcommand selector is not a configuration value, a skipped field is never
/// deserialized, and an unmarked flattened field has no readable key of its
/// own. A flattened field can only contribute the requirements it composes, so
/// it participates exactly when composition is possible.
fn participates(field: &Field, attrs: &FieldAttrs) -> syn::Result<bool> {
    if attrs.cli.is_subcommand() || serde_field_skips_deserialization(&field.attrs)? {
        return Ok(false);
    }
    let is_flattened = serde_field_is_flattened(&field.attrs)?;
    Ok(!is_flattened || composes_children(field, attrs)?)
}

/// Decide whether the field's declared type describes the child requirements.
///
/// A custom deserializer replaces the function Serde uses to read the value,
/// so the child's own `Deserialize` implementation — and the key model the
/// schema derives from it — may never run. Composing those requirements would
/// report leaves the deserializer is free to ignore, so such a field is
/// reported as an opaque leaf: its own key remains the route that supplies it,
/// and no requirement is invented beneath it.
fn composes_children(field: &Field, attrs: &FieldAttrs) -> syn::Result<bool> {
    Ok(attrs.nested && !serde_field_has_custom_deserializer(&field.attrs)?)
}

/// Decide whether a field is shaped so the parser can populate it from a flag.
///
/// Only plain scalar fields take part: a skipped field has no flag, a nested
/// one is populated through its children, and a flattened or skipped-deser
/// field is not keyed by its identifier in the merged JSON.
fn accepts_cli_hint(field: &Field, attrs: &FieldAttrs) -> syn::Result<bool> {
    if !attrs.cli.is_option() || attrs.nested {
        return Ok(false);
    }
    if serde_field_is_flattened(&field.attrs)? {
        return Ok(false);
    }
    Ok(!serde_field_skips_deserialization(&field.attrs)?)
}

/// Build the CLI flag hints accepted for top-level leaves.
///
/// A hint is emitted only when the deserialization key equals the identifier
/// the generated parser uses, because the parser and defaults layers key their
/// JSON by that identifier. Offering a flag for a renamed key would point at a
/// flag that cannot populate it.
fn cli_hints(args: &RequiredSchemaArgs<'_>) -> syn::Result<Vec<TokenStream>> {
    let krate = args.krate;
    let mut hints = Vec::new();
    for (field, attrs) in args.fields.iter().zip(args.field_attrs) {
        let Some(ident) = field.ident.as_ref() else {
            continue;
        };
        let source_name = ident.unraw().to_string();
        if !accepts_cli_hint(field, attrs)? {
            continue;
        }
        let key = serde_deserialized_field_key(field, args.deserialize_rename_all)?;
        if key != source_name {
            continue;
        }
        let Some(metadata) = args
            .cli_metadata
            .iter()
            .find(|metadata| metadata.field_name == source_name)
        else {
            continue;
        };
        let flag = format!("--{}", metadata.long);
        let takes_value = !metadata.is_bool;
        hints.push(quote! {
            #krate::CliFlagHint {
                key: ::std::string::String::from(#key),
                flag: ::std::string::String::from(#flag),
                takes_value: #takes_value,
            }
        });
    }
    Ok(hints)
}

/// Render the route-hint prefix expression.
fn prefix_tokens(struct_attrs: &StructAttrs) -> TokenStream {
    struct_attrs.prefix.as_ref().map_or_else(
        || quote! { ::std::option::Option::None },
        |prefix| quote! { ::std::option::Option::Some(::std::string::String::from(#prefix)) },
    )
}

#[cfg(test)]
mod tests;
