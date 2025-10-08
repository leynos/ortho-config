//! Parsing utilities for the `OrthoConfig` derive macro.
//!
//! Basic compile-check example:
//!
//! ```rust
//! // This trivial example exists to keep doctests compiling in this module.
//! // The parsing helpers below are internal to the macro and exercised by
//! // unit tests; this snippet simply guards against accidental doctest
//! // breakage (e.g., invalid code fences).
//! let _ = 1 + 1;
//! ```

use syn::parenthesized;
use syn::{
    Attribute, Data, DeriveInput, Expr, Fields, GenericArgument, Lit, LitStr, PathArguments, Token,
    Type,
};

#[derive(Default, Clone)]
pub(crate) struct StructAttrs {
    pub prefix: Option<String>,
    pub discovery: Option<DiscoveryAttrs>,
}

#[derive(Default, Clone)]
pub(crate) struct FieldAttrs {
    pub cli_long: Option<String>,
    pub cli_short: Option<char>,
    pub default: Option<Expr>,
    pub merge_strategy: Option<MergeStrategy>,
}

#[derive(Default, Clone)]
pub(crate) struct DiscoveryAttrs {
    pub app_name: Option<String>,
    pub env_var: Option<String>,
    pub config_file_name: Option<String>,
    pub dotfile_name: Option<String>,
    pub project_file_name: Option<String>,
    pub config_cli_long: Option<String>,
    pub config_cli_short: Option<char>,
    pub config_cli_visible: Option<bool>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MergeStrategy {
    Append,
}

impl MergeStrategy {
    pub(crate) fn parse(s: &str, span: proc_macro2::Span) -> Result<Self, syn::Error> {
        match s {
            "append" => Ok(MergeStrategy::Append),
            _ => Err(syn::Error::new(span, "unknown merge_strategy")),
        }
    }
}

/// Iterate all `#[ortho_config(...)]` attributes once and apply a callback.
fn parse_ortho_config<F>(attrs: &[Attribute], mut f: F) -> syn::Result<()>
where
    F: FnMut(&syn::meta::ParseNestedMeta) -> syn::Result<()>,
{
    for attr in attrs.iter().filter(|a| a.path().is_ident("ortho_config")) {
        attr.parse_nested_meta(|meta| f(&meta))?;
    }
    Ok(())
}

/// Consumes an unrecognised key-value or list without recording it.
fn discard_unknown(meta: &syn::meta::ParseNestedMeta) -> syn::Result<()> {
    if meta.input.peek(Token![=]) {
        meta.value()?.parse::<proc_macro2::TokenStream>()?;
    } else if meta.input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in meta.input);
        content.parse::<proc_macro2::TokenStream>()?;
    }
    Ok(())
}

/// Extracts `#[ortho_config(...)]` metadata applied to a struct.
///
/// Only the `prefix` key is currently recognised. Unknown keys are
/// ignored so callers keep compiling when new attributes appear. This
/// improves forwards compatibility at the cost of allowing silent typos.
/// If stricter validation is desired, a custom `compile_error!` guard can
/// reject unexpected keys.
///
/// Used internally by the derive macro to extract configuration metadata
/// from struct-level attributes.
pub(crate) fn parse_struct_attrs(attrs: &[Attribute]) -> Result<StructAttrs, syn::Error> {
    let mut out = StructAttrs::default();
    parse_ortho_config(attrs, |meta| {
        if meta.path.is_ident("prefix") {
            let val = meta.value()?.parse::<Lit>()?;
            if let Lit::Str(s) = val {
                out.prefix = Some(s.value());
            } else {
                return Err(syn::Error::new(val.span(), "prefix must be a string"));
            }
            Ok(())
        } else if meta.path.is_ident("discovery") {
            let mut discovery = out.discovery.take().unwrap_or_default();
            meta.parse_nested_meta(|nested| {
                let Some(ident) = nested.path.get_ident().map(ToString::to_string) else {
                    return discard_unknown(&nested);
                };
                match ident.as_str() {
                    "app_name" => {
                        let s = lit_str(&nested, "app_name")?;
                        discovery.app_name = Some(s.value());
                    }
                    "env_var" => {
                        let s = lit_str(&nested, "env_var")?;
                        discovery.env_var = Some(s.value());
                    }
                    "config_file_name" => {
                        let s = lit_str(&nested, "config_file_name")?;
                        discovery.config_file_name = Some(s.value());
                    }
                    "dotfile_name" => {
                        let s = lit_str(&nested, "dotfile_name")?;
                        discovery.dotfile_name = Some(s.value());
                    }
                    "project_file_name" => {
                        let s = lit_str(&nested, "project_file_name")?;
                        discovery.project_file_name = Some(s.value());
                    }
                    "config_cli_long" => {
                        let s = lit_str(&nested, "config_cli_long")?;
                        discovery.config_cli_long = Some(s.value());
                    }
                    "config_cli_short" => {
                        let c = lit_char(&nested, "config_cli_short")?;
                        discovery.config_cli_short = Some(c);
                    }
                    "config_cli_visible" => {
                        let b = lit_bool(&nested, "config_cli_visible")?;
                        discovery.config_cli_visible = Some(b);
                    }
                    _ => discard_unknown(&nested)?,
                }
                Ok(())
            })?;
            out.discovery = Some(discovery);
            Ok(())
        } else {
            // Unknown attributes are intentionally discarded to preserve
            // backwards compatibility. This allows new keys to be added without
            // breaking callers, but unrecognized attributes will be silently
            // ignored.
            discard_unknown(meta)
        }
    })?;
    Ok(out)
}

/// Parses a literal from a field attribute using `extractor`.
///
/// # Examples
///
/// ```ignore
/// # use syn::meta::ParseNestedMeta;
/// # use syn::{Lit, LitStr};
/// # fn demo(meta: &ParseNestedMeta) -> syn::Result<()> {
/// let s: LitStr = parse_lit(meta, "cli_long", |lit| match lit {
///     Lit::Str(s) => Some(s),
///     _ => None,
/// })?;
/// # Ok(())
/// # }
/// ```rust,ignore
fn parse_lit<T, F>(
    meta: &syn::meta::ParseNestedMeta,
    key: &str,
    extractor: F,
) -> Result<T, syn::Error>
where
    F: FnOnce(Lit) -> Option<T>,
{
    let lit = meta.value()?.parse::<Lit>()?;
    let span = lit.span();
    extractor(lit).ok_or_else(|| {
        let ty = std::any::type_name::<T>()
            .rsplit("::")
            .next()
            .unwrap_or("literal")
            .to_lowercase();
        let ty = match ty.as_str() {
            "litstr" => "string",
            other => other,
        };
        syn::Error::new(span, format!("{key} must be a {ty}"))
    })
}

/// Parses a string literal from a field attribute.
///
/// # Examples
///
/// ```rust
/// // Build a synthetic attribute and visit its nested meta so we can call into
/// // the parsing helper in this crate. This example ensures the documented
/// // function signature stays aligned with the implementation.
/// use syn::Attribute;
/// let attr: Attribute = syn::parse_quote!(#[ortho_config(cli_long = "name")]);
/// attr.parse_nested_meta(|meta| {
///     let s = ortho_config_macros::__doc_lit_str(&meta, "cli_long")?;
///     assert_eq!(s.value(), "name");
///     Ok(())
/// }).unwrap();
/// ```
fn lit_str(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<LitStr, syn::Error> {
    parse_lit(meta, key, |lit| match lit {
        Lit::Str(s) => Some(s),
        _ => None,
    })
}

/// Parses a character literal from a field attribute.
///
/// # Examples
///
/// ```rust,ignore
/// # use syn::meta::ParseNestedMeta;
/// # fn demo(meta: &ParseNestedMeta) -> syn::Result<()> {
/// let c = lit_char(meta, "cli_short")?;
/// assert_eq!(c, 'n');
/// # Ok(())
/// # }
/// ```rust,ignore
fn lit_char(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<char, syn::Error> {
    parse_lit(meta, key, |lit| match lit {
        Lit::Char(c) => Some(c.value()),
        _ => None,
    })
}

fn lit_bool(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<bool, syn::Error> {
    parse_lit(meta, key, |lit| match lit {
        Lit::Bool(b) => Some(b.value),
        _ => None,
    })
}

/// Applies a recognised field attribute, returning `true` if handled.
///
/// # Examples
///
/// ```rust,ignore
/// # use syn::meta::ParseNestedMeta;
/// # fn demo(meta: &ParseNestedMeta) -> syn::Result<()> {
/// let mut out = FieldAttrs::default();
/// if !apply_field_attr(meta, &mut out)? {
///     // unknown attribute
/// }
/// # Ok(())
/// # }
/// ```
fn apply_field_attr(
    meta: &syn::meta::ParseNestedMeta,
    out: &mut FieldAttrs,
) -> Result<bool, syn::Error> {
    match () {
        () if meta.path.is_ident("cli_long") => {
            let s = lit_str(meta, "cli_long")?;
            out.cli_long = Some(s.value());
            Ok(true)
        }
        () if meta.path.is_ident("cli_short") => {
            let c = lit_char(meta, "cli_short")?;
            out.cli_short = Some(c);
            Ok(true)
        }
        () if meta.path.is_ident("default") => {
            out.default = Some(meta.value()?.parse()?);
            Ok(true)
        }
        () if meta.path.is_ident("merge_strategy") => {
            let s = lit_str(meta, "merge_strategy")?;
            out.merge_strategy = Some(MergeStrategy::parse(&s.value(), s.span())?);
            Ok(true)
        }
        () => Ok(false),
    }
}

// Expose a thin wrapper for doctests without leaking internals into the public
// API in normal builds. This allows examples to type-check while keeping
// `lit_str` private outside of tests/doctests.
#[cfg(any(test, doctest))]
#[doc(hidden)]
pub fn __doc_lit_str(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<LitStr, syn::Error> {
    lit_str(meta, key)
}

#[cfg(test)]
mod lit_str_tests {
    use super::*;

    #[test]
    fn lit_str_parses_string_values() {
        let attr: Attribute = syn::parse_quote!(#[ortho_config(cli_long = "name")]);
        attr.parse_nested_meta(|meta| {
            let s = super::__doc_lit_str(&meta, "cli_long")?;
            assert_eq!(s.value(), "name");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn lit_char_parses_char_values() {
        let attr: syn::Attribute = syn::parse_quote!(#[ortho_config(cli_short = 'n')]);
        attr.parse_nested_meta(|meta| {
            let c = super::lit_char(&meta, "cli_short")?;
            assert_eq!(c, 'n');
            Ok(())
        })
        .unwrap();
    }
}

/// Parses field-level `#[ortho_config(...)]` attributes.
///
/// Recognised keys include `cli_long`, `cli_short`, `default` and
/// `merge_strategy`. Unknown keys are ignored, matching
/// [`parse_struct_attrs`] for forwards compatibility. This lenience may
/// permit misspelt attribute names; users wanting stricter validation can
/// insert a manual `compile_error!` guard.
///
/// Used internally by the derive macro to extract configuration metadata
/// from field-level attributes.
pub(crate) fn parse_field_attrs(attrs: &[Attribute]) -> Result<FieldAttrs, syn::Error> {
    let mut out = FieldAttrs::default();
    parse_ortho_config(attrs, |meta| {
        if !apply_field_attr(meta, &mut out)? {
            // Unknown attributes are intentionally discarded to preserve
            // forwards compatibility while still allowing callers to add
            // new keys in future versions.
            discard_unknown(meta)?;
        }
        Ok(())
    })?;
    Ok(out)
}

/// Returns the generic parameter if `ty` is the provided wrapper.
///
/// The check is shallow: it inspects only the outermost path and supports
/// common fully-qualified forms like `std::option::Option<T>`. The function is
/// not recursive.
fn type_inner<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    if let Type::Path(p) = ty {
        // Grab the final two segments (if available) to match paths such as
        // `std::option::Option<T>` or `crate::option::Option<T>` without caring
        // about the full prefix.
        let mut segs = p.path.segments.iter().rev();
        let last = segs.next()?;
        if last.ident != wrapper {
            return None;
        }

        // Ignore the parent segment so crate-relative forms such as
        // `crate::option::Option<T>` and custom module paths match.
        let _ = segs.next();

        if let PathArguments::AngleBracketed(args) = &last.arguments {
            return args.args.first().and_then(|arg| match arg {
                GenericArgument::Type(inner) => Some(inner),
                _ => None,
            });
        }
    }
    None
}

/// Returns the inner type if `ty` is `Option<T>`.
///
/// This uses [`type_inner`], which is **not recursive**. It only inspects the
/// outermost layer, so `Option<Vec<T>>` yields `Vec<T>` rather than `T`.
pub(crate) fn option_inner(ty: &Type) -> Option<&Type> {
    type_inner(ty, "Option")
}

/// Extracts the element type `T` if `ty` is `Vec<T>`.
///
/// Used internally by the derive macro to identify vector fields that
/// require special append merge logic.
pub(crate) fn vec_inner(ty: &Type) -> Option<&Type> {
    type_inner(ty, "Vec")
}

/// Gathers information from the user-provided struct.
///
/// The helper collects the struct identifier, its fields, and all
/// attribute metadata in one pass. Returning these components together
/// keeps the `derive` implementation simple and validates invalid input
/// eagerly so expansion can fail fast.
///
/// The returned tuple contains:
/// - `ident`: the struct identifier
/// - `fields`: the struct's fields
/// - `struct_attrs`: parsed struct-level attributes
/// - `field_attrs`: parsed field-level attributes
pub(crate) fn parse_input(
    input: &DeriveInput,
) -> Result<(syn::Ident, Vec<syn::Field>, StructAttrs, Vec<FieldAttrs>), syn::Error> {
    let ident = input.ident.clone();
    let struct_attrs = parse_struct_attrs(&input.attrs)?;
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => named.named.iter().cloned().collect::<Vec<_>>(),
            _ => {
                return Err(syn::Error::new_spanned(
                    data.struct_token,
                    "OrthoConfig requires named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                ident.clone(),
                "OrthoConfig can only be derived for structs",
            ));
        }
    };

    let mut field_attrs = Vec::new();
    for f in &fields {
        field_attrs.push(parse_field_attrs(&f.attrs)?);
    }
    Ok((ident, fields, struct_attrs, field_attrs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use syn::parse_quote;

    #[test]
    fn parses_struct_and_field_attributes() {
        let input: DeriveInput = parse_quote! {
            #[ortho_config(prefix = "CFG_")]
            struct Demo {
                #[ortho_config(cli_long = "opt", cli_short = 'o', default = 5)]
                field1: Option<u32>,
                #[ortho_config(merge_strategy = "append")]
                field2: Vec<String>,
            }
        };

        let (ident, fields, struct_attrs, field_attrs) = parse_input(&input).expect("parse_input");

        assert_eq!(ident.to_string(), "Demo");
        assert_eq!(fields.len(), 2);
        assert_eq!(struct_attrs.prefix.as_deref(), Some("CFG_"));
        assert_eq!(field_attrs.len(), 2);
        assert_eq!(field_attrs[0].cli_long.as_deref(), Some("opt"));
        assert_eq!(field_attrs[0].cli_short, Some('o'));
        assert!(matches!(
            field_attrs[1].merge_strategy,
            Some(MergeStrategy::Append)
        ));
    }

    #[test]
    fn parses_discovery_attributes() {
        let input: DeriveInput = parse_quote! {
            #[ortho_config(prefix = "CFG_", discovery(
                app_name = "demo",
                env_var = "DEMO_CONFIG",
                config_file_name = "demo.toml",
                dotfile_name = ".demo.toml",
                project_file_name = "demo-config.toml",
                config_cli_long = "config",
                config_cli_short = 'c',
                config_cli_visible = true,
            ))]
            struct Demo {
                value: u32,
            }
        };

        let (_, _, struct_attrs, _) = parse_input(&input).expect("parse_input");
        let discovery = struct_attrs.discovery.expect("discovery attrs");
        assert_eq!(discovery.app_name.as_deref(), Some("demo"));
        assert_eq!(discovery.env_var.as_deref(), Some("DEMO_CONFIG"));
        assert_eq!(discovery.config_file_name.as_deref(), Some("demo.toml"));
        assert_eq!(discovery.dotfile_name.as_deref(), Some(".demo.toml"));
        assert_eq!(
            discovery.project_file_name.as_deref(),
            Some("demo-config.toml"),
        );
        assert_eq!(discovery.config_cli_long.as_deref(), Some("config"));
        assert_eq!(discovery.config_cli_short, Some('c'));
        assert_eq!(discovery.config_cli_visible, Some(true));
    }

    #[rstest]
    #[case::unknown_key(
        parse_quote! {
            #[ortho_config(prefix = "CFG_", unknown = "ignored")]
            struct Demo {
                #[ortho_config(bad_key)]
                field1: String,
            }
        },
        None
    )]
    #[case::unknown_key_with_value(
        parse_quote! {
            #[ortho_config(prefix = "CFG_", unexpected = 42)]
            struct Demo {
                #[ortho_config(cli_long = "f1", extra = true)]
                field1: String,
            }
        },
        Some("f1")
    )]
    #[case::multiple_unknown_keys(
        parse_quote! {
            #[ortho_config(foo, bar, prefix = "CFG_")]
            struct Demo {
                #[ortho_config(baz, qux, cli_long = "f1")]
                field1: String,
            }
        },
        Some("f1")
    )]
    #[case::mixed_order(
        parse_quote! {
            #[ortho_config(alpha, prefix = "CFG_", omega)]
            struct Demo {
                #[ortho_config(beta, cli_long = "f1", gamma)]
                field1: String,
            }
        },
        Some("f1")
    )]
    fn test_unknown_keys_handling(#[case] input: DeriveInput, #[case] cli_long: Option<&str>) {
        let (_ident, fields, struct_attrs, field_attrs) = parse_input(&input).expect("parse_input");

        assert_eq!(fields.len(), 1);
        assert_eq!(struct_attrs.prefix.as_deref(), Some("CFG_"));
        assert_eq!(field_attrs[0].cli_long.as_deref(), cli_long);
    }

    #[rstest]
    #[case(parse_quote!(Option<u32>))]
    #[case(parse_quote!(std::option::Option<u32>))]
    #[case(parse_quote!(core::option::Option<u32>))]
    #[case(parse_quote!(crate::option::Option<u32>))]
    fn option_inner_matches_various_prefixes(#[case] ty: Type) {
        let expected: Type = parse_quote!(u32);
        let inner = option_inner(&ty).expect("should extract");
        assert_eq!(inner, &expected);
    }

    #[rstest]
    #[case(parse_quote!(Vec<u8>))]
    #[case(parse_quote!(std::vec::Vec<u8>))]
    #[case(parse_quote!(alloc::vec::Vec<u8>))]
    #[case(parse_quote!(crate::vec::Vec<u8>))]
    fn vec_inner_matches_various_prefixes(#[case] ty: Type) {
        let expected: Type = parse_quote!(u8);
        let inner = vec_inner(&ty).expect("should extract");
        assert_eq!(inner, &expected);
    }
}
