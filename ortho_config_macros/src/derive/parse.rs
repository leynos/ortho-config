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

use syn::meta::ParseNestedMeta;
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

/// Field-level attributes recognised by `#[derive(OrthoConfig)]`.
///
/// - `cli_long`/`cli_short` override generated CLI flags.
/// - `default` supplies a compile-time default expression when no layer
///   configures the field.
/// - `merge_strategy` selects how collections combine during declarative
///   merges.
/// - `skip_cli` omits the field from CLI parsing whilst leaving declarative
///   merging untouched.
/// - `cli_default_as_absent` treats clap's default value as absent during
///   merge, allowing file/env values to take precedence over CLI defaults.
#[derive(Default, Clone)]
pub(crate) struct FieldAttrs {
    pub cli_long: Option<String>,
    pub cli_short: Option<char>,
    pub default: Option<Expr>,
    pub merge_strategy: Option<MergeStrategy>,
    pub skip_cli: bool,
    pub cli_default_as_absent: bool,
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
    Replace,
    Keyed,
}

impl MergeStrategy {
    pub(crate) fn parse(s: &str, span: proc_macro2::Span) -> Result<Self, syn::Error> {
        match s {
            "append" => Ok(Self::Append),
            "replace" => Ok(Self::Replace),
            "keyed" => Ok(Self::Keyed),
            _ => Err(syn::Error::new(
                span,
                format!(
                    "unknown merge_strategy '{s}'; expected one of \"append\", \"replace\", or \"keyed\""
                ),
            )),
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

fn parse_prefix(meta: &ParseNestedMeta) -> syn::Result<String> {
    let lit = meta.value()?.parse::<Lit>()?;
    match lit {
        Lit::Str(s) => {
            let mut value = s.value();
            if !value.is_empty() && !value.ends_with('_') {
                value.push('_');
            }
            Ok(value)
        }
        other => Err(syn::Error::new(other.span(), "prefix must be a string")),
    }
}

fn parse_discovery_meta(meta: &ParseNestedMeta, discovery: &mut DiscoveryAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| handle_discovery_nested(&nested, discovery))
}

fn handle_discovery_nested(
    nested: &ParseNestedMeta,
    discovery: &mut DiscoveryAttrs,
) -> syn::Result<()> {
    let Some(ident) = nested.path.get_ident().map(ToString::to_string) else {
        return discard_unknown(nested);
    };

    match ident.as_str() {
        "app_name" => assign_str(&mut discovery.app_name, nested, "app_name"),
        "env_var" => assign_str(&mut discovery.env_var, nested, "env_var"),
        "config_file_name" => {
            assign_str(&mut discovery.config_file_name, nested, "config_file_name")
        }
        "dotfile_name" => assign_str(&mut discovery.dotfile_name, nested, "dotfile_name"),
        "project_file_name" => assign_str(
            &mut discovery.project_file_name,
            nested,
            "project_file_name",
        ),
        "config_cli_long" => assign_str(&mut discovery.config_cli_long, nested, "config_cli_long"),
        "config_cli_short" => {
            assign_char(&mut discovery.config_cli_short, nested, "config_cli_short")
        }
        "config_cli_visible" => assign_bool(
            &mut discovery.config_cli_visible,
            nested,
            "config_cli_visible",
        ),
        _ => discard_unknown(nested),
    }
}

fn assign_str(target: &mut Option<String>, nested: &ParseNestedMeta, key: &str) -> syn::Result<()> {
    let value = lit_str(nested, key)?.value();
    *target = Some(value);
    Ok(())
}

fn assign_char(target: &mut Option<char>, nested: &ParseNestedMeta, key: &str) -> syn::Result<()> {
    let value = lit_char(nested, key)?;
    *target = Some(value);
    Ok(())
}

fn assign_bool(target: &mut Option<bool>, nested: &ParseNestedMeta, key: &str) -> syn::Result<()> {
    let value = lit_bool(nested, key)?;
    *target = Some(value);
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
        match meta.path.get_ident().map(ToString::to_string).as_deref() {
            Some("prefix") => {
                let value = parse_prefix(meta)?;
                out.prefix = Some(value);
                Ok(())
            }
            Some("discovery") => {
                let mut discovery = out.discovery.take().unwrap_or_default();
                parse_discovery_meta(meta, &mut discovery)?;
                out.discovery = Some(discovery);
                Ok(())
            }
            _ => discard_unknown(meta),
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
    let literal = meta.value()?.parse::<Lit>()?;
    let span = literal.span();
    extractor(literal).ok_or_else(|| {
        let type_name = std::any::type_name::<T>()
            .rsplit("::")
            .next()
            .unwrap_or("literal")
            .to_lowercase();
        let display_type = match type_name.as_str() {
            "litstr" => "string",
            other => other,
        };
        syn::Error::new(span, format!("{key} must be a {display_type}"))
    })
}

/// Parses a string literal from a field attribute.
///
/// # Examples
///
/// ```rust,ignore
/// // Build a synthetic attribute and visit its nested meta so we can call into
/// // the parsing helper in this crate. The nightly-2025-09-16 toolchain that
/// // backs this repository currently ICEs when compiling the snippet, so the
/// // example is marked `ignore` until the regression is fixed.
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
        () if meta.path.is_ident("skip_cli") => {
            out.skip_cli = true;
            Ok(true)
        }
        () if meta.path.is_ident("cli_default_as_absent") => {
            out.cli_default_as_absent = true;
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
    use anyhow::{Result, anyhow, ensure};

    #[test]
    fn lit_str_parses_string_values() -> Result<()> {
        let attr: Attribute = syn::parse_quote!(#[ortho_config(cli_long = "name")]);
        let mut observed = None;
        attr.parse_nested_meta(|meta| {
            let s = super::__doc_lit_str(&meta, "cli_long")?;
            observed = Some(s.value());
            Ok(())
        })
        .map_err(|err| anyhow!("expected attribute parsing to succeed: {err}"))?;
        let value =
            observed.ok_or_else(|| anyhow!("cli_long attribute callback was not invoked"))?;
        ensure!(value == "name", "unexpected cli_long value: {value}");
        Ok(())
    }

    #[test]
    fn lit_char_parses_char_values() -> Result<()> {
        let attr: syn::Attribute = syn::parse_quote!(#[ortho_config(cli_short = 'n')]);
        let mut observed = None;
        attr.parse_nested_meta(|meta| {
            let c = super::lit_char(&meta, "cli_short")?;
            observed = Some(c);
            Ok(())
        })
        .map_err(|err| anyhow!("expected attribute parsing to succeed: {err}"))?;
        let value =
            observed.ok_or_else(|| anyhow!("cli_short attribute callback was not invoked"))?;
        ensure!(value == 'n', "unexpected cli_short value: {value}");
        Ok(())
    }
}

/// Parses field-level `#[ortho_config(...)]` attributes.
///
/// Recognised keys include `cli_long`, `cli_short`, `default`,
/// `merge_strategy`, and `skip_cli`. Unknown keys are ignored, matching
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

/// Extracts the key and value types if `ty` is `BTreeMap<K, V>`.
///
/// The helper mirrors [`vec_inner`], matching both plain and fully-qualified
/// paths where the final segment is `BTreeMap`.
pub(crate) fn btree_map_inner(ty: &Type) -> Option<(&Type, &Type)> {
    let Type::Path(p) = ty else {
        return None;
    };
    let mut segs = p.path.segments.iter().rev();
    let last = segs.next()?;
    if last.ident != "BTreeMap" {
        return None;
    }
    let _ = segs.next();
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    let mut type_args = args.args.iter().filter_map(|arg| match arg {
        GenericArgument::Type(inner) => Some(inner),
        _ => None,
    });
    let key = type_args.next()?;
    let value = type_args.next()?;
    Some((key, value))
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
    use anyhow::{Result, anyhow, ensure};
    use quote::quote;
    use rstest::rstest;
    use syn::{Attribute, parse_quote};

    /// Helper to assert that a `merge_strategy` attribute is correctly parsed.
    struct MergeStrategyCase<'a> {
        strategy_name: &'a str,
        expected: MergeStrategy,
        struct_name: &'a str,
        field_name: &'a str,
        field_type: &'a proc_macro2::TokenStream,
    }

    fn assert_merge_strategy(case: &MergeStrategyCase<'_>) -> Result<()> {
        let input: DeriveInput = syn::parse_str(&format!(
            r#"
        struct {struct_name} {{
            #[ortho_config(merge_strategy = "{strategy_name}")]
            {field_name}: {field_type},
        }}
        "#,
            struct_name = case.struct_name,
            strategy_name = case.strategy_name,
            field_name = case.field_name,
            field_type = case.field_type,
        ))
        .map_err(|err| anyhow!("failed to parse input: {err}"))?;

        let (_, _, _, attrs_vec) = parse_input(&input).map_err(|err| anyhow!(err))?;
        let attrs = attrs_vec
            .first()
            .ok_or_else(|| anyhow!("missing field attributes"))?;
        ensure!(
            attrs.merge_strategy == Some(case.expected),
            "{strategy} strategy not parsed",
            strategy = case.strategy_name,
        );
        Ok(())
    }

    #[test]
    fn parses_struct_and_field_attributes() -> Result<()> {
        let input: DeriveInput = parse_quote! {
            #[ortho_config(prefix = "CFG_")]
            struct Demo {
                #[ortho_config(cli_long = "opt", cli_short = 'o', default = 5)]
                field1: Option<u32>,
                #[ortho_config(merge_strategy = "append")]
                field2: Vec<String>,
            }
        };

        let (ident, fields, struct_attrs, field_attrs) =
            parse_input(&input).map_err(|err| anyhow!(err))?;

        ensure!(ident == "Demo", "expected Demo ident, got {ident}");
        ensure!(fields.len() == 2, "expected 2 fields, got {}", fields.len());
        ensure!(
            struct_attrs.prefix.as_deref() == Some("CFG_"),
            "expected CFG_ prefix"
        );
        ensure!(field_attrs.len() == 2, "expected 2 field attrs");
        ensure!(
            field_attrs
                .first()
                .and_then(|attrs| attrs.cli_long.as_deref())
                == Some("opt"),
            "expected first cli_long opt"
        );
        ensure!(
            field_attrs.first().and_then(|attrs| attrs.cli_short) == Some('o'),
            "expected first cli_short o"
        );
        ensure!(
            matches!(
                field_attrs.get(1).and_then(|attrs| attrs.merge_strategy),
                Some(MergeStrategy::Append)
            ),
            "expected second field append strategy"
        );
        Ok(())
    }

    #[test]
    fn parses_skip_cli_flag() -> Result<()> {
        let input: DeriveInput = parse_quote! {
            struct Demo {
                #[ortho_config(skip_cli)]
                field: String,
            }
        };

        let (_, fields, _, field_attrs) = parse_input(&input).map_err(|err| anyhow!(err))?;
        ensure!(fields.len() == 1, "expected single field");
        let attrs = field_attrs
            .first()
            .ok_or_else(|| anyhow!("missing field attributes"))?;
        ensure!(attrs.skip_cli, "skip_cli flag was not set");
        Ok(())
    }

    #[test]
    fn parses_cli_default_as_absent_flag() -> Result<()> {
        let input: DeriveInput = parse_quote! {
            struct Demo {
                #[ortho_config(cli_default_as_absent)]
                field: String,
            }
        };

        let (_, fields, _, field_attrs) = parse_input(&input).map_err(|err| anyhow!(err))?;
        ensure!(fields.len() == 1, "expected single field");
        let attrs = field_attrs
            .first()
            .ok_or_else(|| anyhow!("missing field attributes"))?;
        ensure!(
            attrs.cli_default_as_absent,
            "cli_default_as_absent flag was not set"
        );
        Ok(())
    }

    #[test]
    fn parses_discovery_attributes() -> Result<()> {
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

        let (_, _, struct_attrs, _) = parse_input(&input).map_err(|err| anyhow!(err))?;
        let discovery = struct_attrs
            .discovery
            .ok_or_else(|| anyhow!("missing discovery attrs"))?;
        ensure!(
            discovery.app_name.as_deref() == Some("demo"),
            "app_name mismatch"
        );
        ensure!(
            discovery.env_var.as_deref() == Some("DEMO_CONFIG"),
            "env_var mismatch"
        );
        ensure!(
            discovery.config_file_name.as_deref() == Some("demo.toml"),
            "config_file_name mismatch"
        );
        ensure!(
            discovery.dotfile_name.as_deref() == Some(".demo.toml"),
            "dotfile mismatch"
        );
        ensure!(
            discovery.project_file_name.as_deref() == Some("demo-config.toml"),
            "project file mismatch"
        );
        ensure!(
            discovery.config_cli_long.as_deref() == Some("config"),
            "cli long mismatch"
        );
        ensure!(
            discovery.config_cli_short == Some('c'),
            "cli short mismatch"
        );
        ensure!(
            discovery.config_cli_visible == Some(true),
            "visibility mismatch"
        );
        Ok(())
    }

    #[test]
    fn parses_merge_strategy_append() -> Result<()> {
        let field_type = quote!(Vec<String>);
        let case = MergeStrategyCase {
            strategy_name: "append",
            expected: MergeStrategy::Append,
            struct_name: "AppendDemo",
            field_name: "values",
            field_type: &field_type,
        };
        assert_merge_strategy(&case)
    }

    #[test]
    fn parses_merge_strategy_replace() -> Result<()> {
        let field_type = quote!(Vec<String>);
        let case = MergeStrategyCase {
            strategy_name: "replace",
            expected: MergeStrategy::Replace,
            struct_name: "ReplaceDemo",
            field_name: "values",
            field_type: &field_type,
        };
        assert_merge_strategy(&case)
    }

    #[test]
    fn parses_merge_strategy_keyed() -> Result<()> {
        let field_type = quote!(std::collections::BTreeMap<String, u32>);
        let case = MergeStrategyCase {
            strategy_name: "keyed",
            expected: MergeStrategy::Keyed,
            struct_name: "MapDemo",
            field_name: "rules",
            field_type: &field_type,
        };
        assert_merge_strategy(&case)
    }

    #[test]
    fn parses_merge_strategy_invalid() -> Result<()> {
        let invalid: DeriveInput = parse_quote! {
            struct InvalidDemo {
                #[ortho_config(merge_strategy = "unknown")]
                values: Vec<String>,
            }
        };
        let err = parse_input(&invalid)
            .err()
            .ok_or_else(|| anyhow!("expected merge strategy error"))?;
        ensure!(
            err.to_string().contains("unknown merge_strategy"),
            "unexpected error message: {err}",
        );
        Ok(())
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
    fn test_unknown_keys_handling(
        #[case] input: DeriveInput,
        #[case] cli_long: Option<&str>,
    ) -> Result<()> {
        let (_ident, fields, struct_attrs, field_attrs) =
            parse_input(&input).map_err(|err| anyhow!(err))?;

        ensure!(fields.len() == 1, "expected single field");
        ensure!(
            struct_attrs.prefix.as_deref() == Some("CFG_"),
            "expected CFG_ prefix"
        );
        let parsed = field_attrs
            .first()
            .and_then(|attrs| attrs.cli_long.as_deref());
        ensure!(
            parsed == cli_long,
            "cli_long mismatch: {parsed:?} != {cli_long:?}"
        );
        Ok(())
    }

    #[rstest]
    #[case::missing_suffix("APP", "APP_")]
    #[case::with_suffix("APP_", "APP_")]
    #[case::empty("", "")]
    fn struct_prefix_normalises_trailing_underscore(
        #[case] raw: &str,
        #[case] expected: &str,
    ) -> Result<()> {
        let lit = syn::LitStr::new(raw, proc_macro2::Span::call_site());
        let attr: Attribute = syn::parse_quote!(#[ortho_config(prefix = #lit)]);
        let attrs = parse_struct_attrs(&[attr]).map_err(|err| anyhow!(err))?;

        ensure!(
            attrs.prefix.as_deref() == Some(expected),
            "prefix normalisation mismatch"
        );
        Ok(())
    }

    #[rstest]
    #[case(parse_quote!(Option<u32>))]
    #[case(parse_quote!(std::option::Option<u32>))]
    #[case(parse_quote!(core::option::Option<u32>))]
    #[case(parse_quote!(crate::option::Option<u32>))]
    fn option_inner_matches_various_prefixes(#[case] ty: Type) -> Result<()> {
        let expected: Type = parse_quote!(u32);
        let inner = option_inner(&ty).ok_or_else(|| anyhow!("expected Option"))?;
        ensure!(inner == &expected, "expected {expected:?}, got {inner:?}");
        Ok(())
    }

    #[rstest]
    #[case(parse_quote!(Vec<u8>))]
    #[case(parse_quote!(std::vec::Vec<u8>))]
    #[case(parse_quote!(alloc::vec::Vec<u8>))]
    #[case(parse_quote!(crate::vec::Vec<u8>))]
    fn vec_inner_matches_various_prefixes(#[case] ty: Type) -> Result<()> {
        let expected: Type = parse_quote!(u8);
        let inner = vec_inner(&ty).ok_or_else(|| anyhow!("expected Vec"))?;
        ensure!(inner == &expected, "expected {expected:?}, got {inner:?}");
        Ok(())
    }

    #[rstest]
    #[case::std(
        parse_quote!(std::collections::BTreeMap<String, u8>),
        parse_quote!(String),
        parse_quote!(u8),
    )]
    #[case::alloc(
        parse_quote!(alloc::collections::BTreeMap<u16, (u8, u8)>),
        parse_quote!(u16),
        parse_quote!((u8, u8)),
    )]
    #[case::crate_prefix(
        parse_quote!(crate::collections::BTreeMap<String, Vec<Option<u8>>>),
        parse_quote!(String),
        parse_quote!(Vec<Option<u8>>),
    )]
    fn btree_map_inner_matches_various_prefixes(
        #[case] ty: Type,
        #[case] expected_key: Type,
        #[case] expected_value: Type,
    ) -> Result<()> {
        let (key, value) = btree_map_inner(&ty).ok_or_else(|| anyhow!("expected BTreeMap"))?;
        ensure!(
            key == &expected_key,
            "key mismatch: {key:?} vs {expected_key:?}"
        );
        ensure!(
            value == &expected_value,
            "value mismatch: {value:?} vs {expected_value:?}",
        );
        Ok(())
    }
}
