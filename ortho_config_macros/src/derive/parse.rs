//! Parsing utilities for the `OrthoConfig` derive macro.

use syn::{Attribute, Data, DeriveInput, Expr, Fields, GenericArgument, Lit, PathArguments, Type};

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

pub(crate) fn parse_struct_attrs(attrs: &[Attribute]) -> Result<StructAttrs, syn::Error> {
    let mut out = StructAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("ortho_config") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("prefix") {
                let val = meta.value()?.parse::<Lit>()?;
                if let Lit::Str(s) = val {
                    out.prefix = Some(s.value());
                } else {
                    return Err(syn::Error::new(val.span(), "prefix must be a string"));
                }
            }
            Ok(())
        })?;
    }
    Ok(out)
}

pub(crate) fn parse_field_attrs(attrs: &[Attribute]) -> Result<FieldAttrs, syn::Error> {
    let mut out = FieldAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("ortho_config") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("cli_long") {
                let val = meta.value()?.parse::<Lit>()?;
                if let Lit::Str(s) = val {
                    out.cli_long = Some(s.value());
                } else {
                    return Err(syn::Error::new(val.span(), "cli_long must be a string"));
                }
            } else if meta.path.is_ident("cli_short") {
                let val = meta.value()?.parse::<Lit>()?;
                if let Lit::Char(c) = val {
                    out.cli_short = Some(c.value());
                } else {
                    return Err(syn::Error::new(val.span(), "cli_short must be a char"));
                }
            } else if meta.path.is_ident("default") {
                let expr = meta.value()?.parse::<Expr>()?;
                out.default = Some(expr);
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
            }
            Ok(())
        })?;
    }
    Ok(out)
}

pub(crate) fn option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            if seg.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn vec_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            if seg.ident == "Vec" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

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
}
