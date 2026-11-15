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

/// Clap default forms retained so generation can reproduce clap parsing semantics.
///
/// `Value` carries parser metadata because its string expression must be parsed
/// into the field type; the `_t` forms already contain typed Rust expressions.
#[derive(Clone)]
pub(crate) enum ClapInferredDefault {
    /// A string default that must be converted using the field parser.
    Value(Box<ClapDefaultValue>),
    /// A typed single-value default from `default_value_t`.
    ValueT(Expr),
    /// A typed collection default from `default_values_t`.
    ValuesT(Expr),
}

/// Parser metadata retained for a clap `default_value` expression.
#[derive(Clone)]
pub(crate) struct ClapDefaultValue {
    /// String expression passed to clap before typed conversion.
    pub value: Expr,
    /// Explicit parser expression that defines conversion for the leaf value.
    pub value_parser: Option<Expr>,
    /// Whether clap should parse the leaf as a `ValueEnum`.
    pub value_enum: bool,
    /// Delimiter expression used when one string supplies several values.
    pub value_delimiter: Option<Expr>,
    /// Case-sensitivity setting retained for enum parsing.
    pub ignore_case: Option<Expr>,
    /// Element type used when constructing the generated typed default.
    pub leaf_type: Type,
    /// Scalar, optional, or vector shape required by generated conversion.
    pub shape: ClapDefaultValueShape,
}

/// Parser options collected before they are attached to a string default.
#[derive(Default)]
struct ClapDefaultHints {
    /// Explicit conversion logic for the default string.
    value_parser: Option<Expr>,
    /// Whether the field uses clap enum conversion.
    value_enum: bool,
    /// Separator used to split a default string into collection values.
    value_delimiter: Option<Expr>,
    /// Case handling retained for enum conversion.
    ignore_case: Option<Expr>,
}

/// Recognized default keys, separated from their parsed expressions.
#[derive(Clone, Copy)]
enum ClapDefaultKind {
    /// A string value requiring typed parsing.
    Value,
    /// A typed expression supplying one value.
    ValueT,
    /// A typed expression supplying multiple values.
    ValuesT,
    /// Any nested key that is not a clap default key.
    Other,
}

/// Stores the first default and rejects another key that would override it.
///
/// # Examples
///
/// The first parsed default fills an empty slot; a second default returns a
/// source-spanned duplicate-default error.
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

/// Maps a nested clap key to its default form; unrelated keys remain `Other`.
///
/// # Examples
///
/// `default_values_t` classifies as `ValuesT`, while `value_parser` classifies
/// as `Other` and remains available as parser metadata.
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

/// Parses a recognized default key, leaving unrelated metadata for its caller.
///
/// Syn parse errors retain the source span of the malformed expression.
///
/// # Examples
///
/// `default_value_t = 3` yields a typed `ValueT` expression; an unrelated key
/// yields `None` without consuming its value.
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

/// Parses default keys and parser hints from one clap nested meta item.
///
/// String and typed defaults are mutually exclusive across the attribute.
/// Parser hints are retained for string defaults; other key-value/list forms
/// are consumed so clap options unrelated to inference remain accepted.
/// Malformed recognised values and duplicate defaults return source-spanned
/// errors.
///
/// # Examples
///
/// Parsing `default_value_t = 3` stores a typed default; parsing another
/// default afterwards returns the duplicate-default diagnostic.
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

/// Reads one clap attribute, ignoring non-list metadata and propagating parse errors.
///
/// # Examples
///
/// A list attribute containing `default_value = "7"` records a string default;
/// a path-style attribute contributes no inferred default.
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
