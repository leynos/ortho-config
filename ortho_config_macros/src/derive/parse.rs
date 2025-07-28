//! Parsing utilities for the `OrthoConfig` derive macro.

use syn::parenthesized;
use syn::{
    Attribute, Data, DeriveInput, Expr, Fields, GenericArgument, Lit, PathArguments, Token, Type,
};

#[derive(Default)]
pub(crate) struct StructAttrs {
    pub prefix: Option<String>,
}

#[derive(Default)]
pub(crate) struct FieldAttrs {
    pub cli_long: Option<String>,
    pub cli_short: Option<char>,
    pub default: Option<Expr>,
    pub merge_strategy: Option<MergeStrategy>,
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
        if meta.path.is_ident("cli_long") {
            let val = meta.value()?.parse::<Lit>()?;
            if let Lit::Str(s) = val {
                out.cli_long = Some(s.value());
            } else {
                return Err(syn::Error::new(val.span(), "cli_long must be a string"));
            }
            Ok(())
        } else if meta.path.is_ident("cli_short") {
            let val = meta.value()?.parse::<Lit>()?;
            if let Lit::Char(c) = val {
                out.cli_short = Some(c.value());
            } else {
                return Err(syn::Error::new(val.span(), "cli_short must be a char"));
            }
            Ok(())
        } else if meta.path.is_ident("default") {
            let expr = meta.value()?.parse::<Expr>()?;
            out.default = Some(expr);
            Ok(())
        } else if meta.path.is_ident("merge_strategy") {
            let val = meta.value()?.parse::<Lit>()?;
            if let Lit::Str(s) = val {
                out.merge_strategy = Some(MergeStrategy::parse(&s.value(), s.span())?);
            } else {
                return Err(syn::Error::new(
                    val.span(),
                    "merge_strategy must be a string",
                ));
            }
            Ok(())
        } else {
            // Ignore unknown attributes so that future versions can add new
            // keys without breaking callers.
            discard_unknown(meta)
        }
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

        // The immediate parent segment may be `option` or `vec`. If absent,
        // assume a shorthand like `Option<T>`.
        if let Some(prev) = segs.next() {
            let expected = match wrapper {
                "Option" => "option",
                "Vec" => "vec",
                _ => "",
            };
            if !expected.is_empty() && prev.ident != expected {
                return None;
            }
        }

        if let PathArguments::AngleBracketed(args) = &last.arguments {
            if let Some(GenericArgument::Type(inner)) = args.args.first() {
                return Some(inner);
            }
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
}
