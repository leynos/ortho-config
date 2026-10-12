//! Inference of clap default expressions from `#[arg(...)]` attributes.
//!
//! These helpers read `default_value`, `default_value_t`, and
//! `default_values_t` together with the parser hints that qualify them
//! (`value_parser`, `value_enum`, `value_delimiter`, `ignore_case`). The
//! inferred value feeds the generated defaults struct when
//! `cli_default_as_absent` is active and no explicit
//! `#[ortho_config(default = ...)]` is given.

use syn::{Expr, Type};

use super::clap_attrs::is_clap_attribute;
use super::type_utils::{ClapDefaultValueShape, clap_default_value_type};

#[derive(Clone)]
pub(crate) enum ClapInferredDefault {
    Value(Box<ClapDefaultValue>),
    ValueT(Expr),
    ValuesT(Expr),
}

/// Parser metadata retained for a clap `default_value` expression.
#[derive(Clone)]
pub(crate) struct ClapDefaultValue {
    pub value: Expr,
    pub value_parser: Option<Expr>,
    pub value_enum: bool,
    pub value_delimiter: Option<Expr>,
    pub ignore_case: Option<Expr>,
    pub leaf_type: Type,
    pub shape: ClapDefaultValueShape,
}

#[derive(Default)]
struct ClapDefaultHints {
    value_parser: Option<Expr>,
    value_enum: bool,
    value_delimiter: Option<Expr>,
    ignore_case: Option<Expr>,
}

#[derive(Clone, Copy)]
enum ClapDefaultKind {
    Value,
    ValueT,
    ValuesT,
    Other,
}

fn assign_default_expr(
    meta: &syn::meta::ParseNestedMeta<'_>,
    default_slot: &mut Option<ClapInferredDefault>,
    parsed_expr: ClapInferredDefault,
) -> syn::Result<()> {
    if default_slot.is_some() {
        return Err(syn::Error::new_spanned(
            &meta.path,
            "duplicate clap default override",
        ));
    }
    *default_slot = Some(parsed_expr);
    Ok(())
}

fn classify_default_kind(meta: &syn::meta::ParseNestedMeta<'_>) -> ClapDefaultKind {
    if meta.path.is_ident("default_value") {
        return ClapDefaultKind::Value;
    }
    if meta.path.is_ident("default_value_t") {
        return ClapDefaultKind::ValueT;
    }
    if meta.path.is_ident("default_values_t") {
        return ClapDefaultKind::ValuesT;
    }
    ClapDefaultKind::Other
}

fn parse_default_expr(
    meta: &syn::meta::ParseNestedMeta<'_>,
    kind: ClapDefaultKind,
) -> syn::Result<Option<ClapInferredDefault>> {
    let parsed = match kind {
        ClapDefaultKind::Value => ClapInferredDefault::Value(Box::new(ClapDefaultValue {
            value: meta.value()?.parse::<Expr>()?,
            value_parser: None,
            value_enum: false,
            value_delimiter: None,
            ignore_case: None,
            leaf_type: syn::parse_quote! { () },
            shape: ClapDefaultValueShape::Scalar,
        })),
        ClapDefaultKind::ValueT => ClapInferredDefault::ValueT(meta.value()?.parse::<Expr>()?),
        ClapDefaultKind::ValuesT => ClapInferredDefault::ValuesT(meta.value()?.parse::<Expr>()?),
        ClapDefaultKind::Other => return Ok(None),
    };
    Ok(Some(parsed))
}

/// Parses clap default-related keys from a nested meta item.
///
/// Recognized keys:
///
/// - `default_value = "..."`
/// - `default_value_t = <expr>`
/// - `default_values_t = <expr>`
///
/// Duplicate defaults (including mixed forms) produce a compile-time error.
fn parse_default_from_meta(
    meta: &syn::meta::ParseNestedMeta<'_>,
    existing_default: &mut Option<ClapInferredDefault>,
    hints: &mut ClapDefaultHints,
) -> syn::Result<()> {
    if let Some(parsed) = parse_default_expr(meta, classify_default_kind(meta))? {
        return assign_default_expr(meta, existing_default, parsed);
    }

    if meta.path.is_ident("value_parser") {
        hints.value_parser = Some(meta.value()?.parse()?);
        return Ok(());
    }
    if meta.path.is_ident("value_enum") {
        hints.value_enum = true;
        return Ok(());
    }
    if meta.path.is_ident("value_delimiter") {
        hints.value_delimiter = Some(meta.value()?.parse()?);
        return Ok(());
    }
    if meta.path.is_ident("ignore_case") {
        hints.ignore_case = Some(meta.value()?.parse()?);
        return Ok(());
    }

    if meta.input.peek(syn::Token![=]) {
        let value = meta.value()?;
        let _: Expr = value.parse()?;
    } else if meta.input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in meta.input);
        content.parse::<proc_macro2::TokenStream>()?;
    }
    Ok(())
}

fn clap_default_value_from_attribute(
    attr: &syn::Attribute,
    existing_default: &mut Option<ClapInferredDefault>,
    hints: &mut ClapDefaultHints,
) -> syn::Result<()> {
    let syn::Meta::List(list) = &attr.meta else {
        return Ok(());
    };

    list.parse_nested_meta(|meta| parse_default_from_meta(&meta, existing_default, hints))
}

/// Returns the typed default expression inferred from clap attributes, if any.
///
/// The generated defaults struct consumes these inferred values and
/// materializes field-level defaults during code generation.
pub(crate) fn clap_default_value(field: &syn::Field) -> syn::Result<Option<ClapInferredDefault>> {
    let mut default_expr: Option<ClapInferredDefault> = None;
    let mut hints = ClapDefaultHints::default();
    for attr in field.attrs.iter().filter(|attr| is_clap_attribute(attr)) {
        clap_default_value_from_attribute(attr, &mut default_expr, &mut hints)?;
    }
    if let Some(ClapInferredDefault::Value(default)) = default_expr.as_mut() {
        let inferred_type = clap_default_value_type(&field.ty).map_err(|unsupported| {
            syn::Error::new_spanned(
                field,
                format!(
                    "clap `default_value` inference for `cli_default_as_absent` does not support {unsupported}; use a scalar, `Option<T>`, or `Vec<T>`, or provide `#[ortho_config(default = ...)]`"
                ),
            )
        })?;
        default.value_parser = hints.value_parser;
        default.value_enum = hints.value_enum;
        default.value_delimiter = hints.value_delimiter;
        default.ignore_case = hints.ignore_case;
        default.leaf_type = inferred_type.leaf;
        default.shape = inferred_type.shape;
    }
    Ok(default_expr)
}
