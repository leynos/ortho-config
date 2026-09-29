//! Parsing helpers for clap field and variant attributes.
//!
//! These helpers extract metadata from `#[arg(...)]` and `#[clap(...)]`
//! attributes without taking a dependency on clap itself.

use syn::{Expr, Type};

use super::type_utils::{ClapDefaultValueShape, clap_default_value_type};

/// Returns `true` when the attribute is `#[arg(...)]` or `#[clap(...)]`.
pub(crate) fn is_clap_attribute(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("arg") || attr.path().is_ident("clap")
}

/// Returns `true` when the attribute is `#[command(...)]` or `#[clap(...)]`.
pub(crate) fn is_clap_command_attribute(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("command") || attr.path().is_ident("clap")
}

/// Parse a clap argument `id = "..."` override from a nested meta item.
///
/// When the meta item is not `id`, this function is a no-op (but will still
/// consume any `= <value>` tokens to keep parsing in sync). When an `id` value
/// is supplied it is stored in `existing_id`, and a duplicate `id` triggers a
/// `syn::Error` with the same message used elsewhere in the derive.
pub(crate) fn parse_id_from_meta(
    meta: &syn::meta::ParseNestedMeta<'_>,
    existing_id: &mut Option<syn::LitStr>,
) -> syn::Result<()> {
    if !meta.path.is_ident("id") {
        if meta.input.peek(syn::Token![=]) {
            let value = meta.value()?;
            let _: syn::Expr = value.parse()?;
        }
        return Ok(());
    }

    if existing_id.is_some() {
        return Err(syn::Error::new_spanned(
            &meta.path,
            "duplicate clap argument `id` override",
        ));
    }

    let Ok(value) = meta.value() else {
        return Ok(());
    };
    let lit: syn::LitStr = value.parse().map_err(|_| {
        syn::Error::new_spanned(&meta.path, "clap argument `id` must be a string literal")
    })?;
    *existing_id = Some(lit);
    Ok(())
}

/// Merge an `id` override from one `arg` or `clap` attribute into the field's
/// accumulated identifier, rejecting malformed nested metadata.
pub(crate) fn clap_arg_id_from_attribute(
    attr: &syn::Attribute,
    existing_id: &mut Option<syn::LitStr>,
) -> syn::Result<()> {
    let syn::Meta::List(list) = &attr.meta else {
        return Ok(());
    };

    list.parse_nested_meta(|meta| parse_id_from_meta(&meta, existing_id))
}

/// Return a field's explicit clap identifier, if an `id = "..."` override exists.
///
/// Attributes are visited in source order and duplicate overrides are errors,
/// so callers receive at most one unambiguous identifier.
pub(crate) fn clap_arg_id(field: &syn::Field) -> syn::Result<Option<String>> {
    let mut arg_id: Option<syn::LitStr> = None;
    for attr in field.attrs.iter().filter(|attr| is_clap_attribute(attr)) {
        clap_arg_id_from_attribute(attr, &mut arg_id)?;
    }
    Ok(arg_id.map(|lit| lit.value()))
}

/// Consume the value or parenthesized tokens of an unrelated clap key so the
/// nested-meta parser can continue without interpreting clap's full grammar.
fn consume_unknown_meta(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<()> {
    if meta.input.peek(syn::Token![=]) {
        let value = meta.value()?;
        let _: syn::Expr = value.parse()?;
    } else if meta.input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in meta.input);
        content.parse::<proc_macro2::TokenStream>()?;
    }
    Ok(())
}

/// Parse a clap command `name = "..."` override from an enum variant.
pub(crate) fn clap_variant_name(variant: &syn::Variant) -> syn::Result<Option<syn::LitStr>> {
    let mut name = None;
    for attr in variant
        .attrs
        .iter()
        .filter(|attr| is_clap_command_attribute(attr))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                name = Some(lit);
                return Ok(());
            }
            consume_unknown_meta(&meta)
        })?;
    }
    Ok(name)
}

/// Detect whether a struct field is a clap subcommand selector.
pub(crate) fn clap_field_is_subcommand(field: &syn::Field) -> syn::Result<bool> {
    let mut is_subcommand = false;
    for attr in field
        .attrs
        .iter()
        .filter(|attr| is_clap_command_attribute(attr))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("subcommand") {
                is_subcommand = true;
                return Ok(());
            }
            consume_unknown_meta(&meta)
        })?;
    }
    Ok(is_subcommand)
}

/// Default syntax retained for generated configuration defaults. Typed clap
/// expressions stay intact, while string values carry parser details for later
/// conversion using the field's inferred scalar type.
#[derive(Clone)]
pub(crate) enum ClapInferredDefault {
    /// A string-like default requiring clap's parser metadata for conversion.
    Value(Box<ClapDefaultValue>),
    /// A typed single-value default expression from `default_value_t`.
    ValueT(Expr),
    /// A typed multi-value default expression from `default_values_t`.
    ValuesT(Expr),
}

/// Parser metadata retained for a clap `default_value` expression.
#[derive(Clone)]
pub(crate) struct ClapDefaultValue {
    /// The source expression for clap's string-valued default.
    pub value: Expr,
    /// Explicit parser expression, preserved so generated code uses clap's conversion.
    pub value_parser: Option<Expr>,
    /// Whether clap should parse the value through its `ValueEnum` implementation.
    pub value_enum: bool,
    /// Delimiter expression used when one default string represents several values.
    pub value_delimiter: Option<Expr>,
    /// Case-insensitive parsing hint passed through to clap's value parser.
    pub ignore_case: Option<Expr>,
    /// Scalar field type that clap must parse before optional or collection wrapping.
    pub leaf_type: Type,
    /// Field container shape used to materialize the parsed value correctly.
    pub shape: ClapDefaultValueShape,
}

/// Clap parsing options collected alongside a string default and applied after
/// the field type determines the parser's expected leaf value.
#[derive(Default)]
struct ClapDefaultHints {
    /// Explicit clap parser expression, if supplied by the field attribute.
    value_parser: Option<Expr>,
    /// Whether enum values should use clap's `ValueEnum` conversion.
    value_enum: bool,
    /// Delimiter used to split a string default into multiple values.
    value_delimiter: Option<Expr>,
    /// Whether value matching should ignore letter case.
    ignore_case: Option<Expr>,
}

/// Classification of clap metadata keys before parsing their value expressions.
#[derive(Clone, Copy)]
enum ClapDefaultKind {
    /// `default_value`, whose string expression needs type and parser hints.
    Value,
    /// `default_value_t`, whose expression is already typed by the caller.
    ValueT,
    /// `default_values_t`, whose expression supplies a typed sequence.
    ValuesT,
    /// Any key unrelated to the three supported default forms.
    Other,
}

/// Store one inferred default, rejecting a second form so mixed clap defaults
/// cannot silently select a precedence based on attribute order.
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

/// Map a clap key to the default representation that preserves its type semantics.
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

/// Parse the value expression for a recognized default key; unrelated keys are
/// left for hint collection or generic consumption.
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

/// Collect default and parser metadata from one list-form clap attribute.
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
/// materializes field-level defaults during code generation. The caller asks
/// for inference only when no explicit `#[ortho_config(default = ...)]` exists,
/// so the `OrthoConfig` default takes precedence over clap's default. String
/// defaults retain clap parser hints; typed defaults retain their expressions.
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

/// Reject `#[ortho_config(...)]` attributes on subcommand-selector fields.
///
/// The `#[command(subcommand)]` selector is a clap concern; combining it with
/// `#[ortho_config]` annotations is meaningless because the field does not
/// participate in config merging or default generation.
pub(crate) fn reject_subcommand_ortho_config_attrs(field: &syn::Field) -> syn::Result<()> {
    for attr in field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("ortho_config"))
    {
        if let syn::Meta::List(list) = &attr.meta
            && list.tokens.is_empty()
        {
            return Err(syn::Error::new_spanned(
                attr,
                "#[command(subcommand)] fields cannot be combined with #[ortho_config()]; remove the conflicting attribute",
            ));
        }
        attr.parse_nested_meta(|meta| {
            let option = meta
                .path
                .get_ident()
                .map_or_else(|| "this option".to_owned(), ToString::to_string);
            Err(meta.error(format!(
                "#[command(subcommand)] fields cannot be combined with \
                 #[ortho_config({option})]; remove the conflicting attribute"
            )))
        })?;
    }
    Ok(())
}
