//! Documentation-related attribute parsing.

use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;
use syn::{LitBool, Token};

use super::doc_attrs_common::{parse_example_meta, parse_link_meta, parse_note_meta};
use super::doc_types::{
    DocFieldAttrs, DocStructAttrs, HeadingOverrides, PrecedenceAttrs, WindowsAttrs,
};
use super::literals::{lit_char, lit_str};
use super::{
    FieldAttrs, discard_unknown, parse_behaviour_meta, parse_output_meta, parse_renderer_meta,
};

/// Applies one documentation setting to the struct model.
///
/// Returns `false` without changing `out` when the key belongs to another
/// attribute family; recognized keys propagate malformed-value errors.
pub(crate) fn apply_struct_doc_attr(
    meta: &ParseNestedMeta,
    out: &mut DocStructAttrs,
) -> syn::Result<bool> {
    let Some(ident) = meta.path.get_ident() else {
        return Ok(false);
    };
    let key = ident.to_string();
    let recognised = match key.as_str() {
        "about_id" => set_lit_string(meta, "about_id", &mut out.about_id),
        "synopsis_id" => set_lit_string(meta, "synopsis_id", &mut out.synopsis_id),
        "bin_name" => set_lit_string(meta, "bin_name", &mut out.bin_name),
        "example" => parse_example_meta(meta).map(|example| out.examples.push(example)),
        "link" => parse_link_meta(meta).map(|link| out.links.push(link)),
        "note" => parse_note_meta(meta).map(|note| out.notes.push(note)),
        "behavior" => Err(misspelt_behaviour(meta)),
        _ => return apply_nested_struct_attr(meta, key.as_str(), out),
    };
    recognised.map(|()| true)
}

/// Stores the string literal named by `key` in `slot`.
fn set_lit_string(meta: &ParseNestedMeta, key: &str, slot: &mut Option<String>) -> syn::Result<()> {
    *slot = Some(lit_str(meta, key)?.value());
    Ok(())
}

/// The diagnostic for the American spelling of the struct-level `behaviour` key.
fn misspelt_behaviour(meta: &ParseNestedMeta) -> syn::Error {
    syn::Error::new(
        meta.path.span(),
        concat!(
            "unknown attribute `behavior`; use the en-GB spelling `behaviour` ",
            "(declares non-interactive execution and mutation-boundary metadata)"
        ),
    )
}

/// Applies one attribute whose value is a nested group.
///
/// Each group may be declared more than once, so an earlier declaration is
/// carried into the parse and only replaced once the new group is valid.
fn apply_nested_struct_attr(
    meta: &ParseNestedMeta,
    key: &str,
    out: &mut DocStructAttrs,
) -> syn::Result<bool> {
    let recognised = match key {
        "headings" => parse_headings_group(meta, out),
        "precedence" => parse_group(&mut out.precedence, |group| {
            parse_precedence_meta(meta, group)
        }),
        "windows" => parse_group(&mut out.windows, |group| parse_windows_meta(meta, group)),
        "behaviour" => parse_group(&mut out.behaviour, |group| {
            parse_behaviour_meta(meta, group)
        }),
        "renderer" => parse_renderer_meta(meta, out.renderer.get_or_insert_default()),
        "output" => parse_output_meta(meta, out.output.get_or_insert_default()),
        _ => return Ok(false),
    };
    recognised.map(|()| true)
}

/// Parses `headings(...)` over a copy of the earlier overrides.
fn parse_headings_group(meta: &ParseNestedMeta, out: &mut DocStructAttrs) -> syn::Result<()> {
    let mut headings = out.headings.clone();
    parse_headings_meta(meta, &mut headings)?;
    out.headings = headings;
    Ok(())
}

/// Parses a repeatable group into its slot, starting from any earlier value.
///
/// The earlier value is taken out of the slot, so a malformed group leaves it
/// empty rather than half-updated.
fn parse_group<T: Default>(
    slot: &mut Option<T>,
    parse: impl FnOnce(&mut T) -> syn::Result<()>,
) -> syn::Result<()> {
    let mut group = slot.take().unwrap_or_default();
    parse(&mut group)?;
    *slot = Some(group);
    Ok(())
}
/// Applies one documentation setting to a field's parsed attribute model.
///
/// Unknown keys are left for the caller to route to other attribute parsers,
/// while recognized keys either update `out` or return a syntax error.
pub(crate) fn apply_field_doc_attr(
    meta: &ParseNestedMeta,
    out: &mut FieldAttrs,
) -> syn::Result<bool> {
    let Some(ident) = meta.path.get_ident() else {
        return Ok(false);
    };
    let key = ident.to_string();
    let recognised = match key.as_str() {
        "help_id" => set_lit_string(meta, "help_id", &mut out.doc.help_id),
        "long_help_id" => set_lit_string(meta, "long_help_id", &mut out.doc.long_help_id),
        "value" => parse_value_meta(meta, &mut out.doc),
        "deprecated" => parse_deprecated_meta(meta, &mut out.doc),
        "required" => parse_bool(meta).map(|required| out.doc.required = Some(required)),
        "env" => parse_env_meta(meta, &mut out.doc),
        "file" => parse_file_meta(meta, &mut out.doc),
        "cli" => parse_cli_meta(meta, out),
        "example" => parse_example_meta(meta).map(|example| out.doc.examples.push(example)),
        "link" => parse_link_meta(meta).map(|link| out.doc.links.push(link)),
        "note" => parse_note_meta(meta).map(|note| out.doc.notes.push(note)),
        "behaviour" | "behavior" => Err(struct_level_behaviour(meta)),
        _ => return Ok(false),
    };
    recognised.map(|()| true)
}

/// The diagnostic for `behaviour(...)` written on a field.
fn struct_level_behaviour(meta: &ParseNestedMeta) -> syn::Error {
    syn::Error::new(
        meta.path.span(),
        "behaviour(...) is a struct-level attribute",
    )
}
/// Parses heading overrides while retaining the caller's earlier values for
/// headings omitted by this attribute.
fn parse_headings_meta(meta: &ParseNestedMeta, headings: &mut HeadingOverrides) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| set_heading_field(&nested, headings))
}

/// Sets the appropriate `HeadingOverrides` field based on the key.
fn set_heading_field(nested: &ParseNestedMeta, headings: &mut HeadingOverrides) -> syn::Result<()> {
    let Some(ident) = nested.path.get_ident() else {
        return discard_unknown(nested);
    };
    let key = ident.to_string();
    let field_ref = match key.as_str() {
        "name" => &mut headings.name,
        "synopsis" => &mut headings.synopsis,
        "description" => &mut headings.description,
        "options" => &mut headings.options,
        "environment" => &mut headings.environment,
        "files" => &mut headings.files,
        "precedence" => &mut headings.precedence,
        "exit_status" => &mut headings.exit_status,
        "examples" => &mut headings.examples,
        "see_also" => &mut headings.see_also,
        _ => return discard_unknown(nested),
    };
    *field_ref = Some(lit_str(nested, &key)?.value());
    Ok(())
}
/// Parses precedence ordering and its localized rationale identifier without
/// changing the order supplied by the author.
fn parse_precedence_meta(
    meta: &ParseNestedMeta,
    precedence: &mut PrecedenceAttrs,
) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("order") {
            precedence.order = parse_string_array(&nested, "order")?;
            return Ok(());
        }
        if nested.path.is_ident("rationale_id") {
            precedence.rationale_id = Some(lit_str(&nested, "rationale_id")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })
}
/// Parses PowerShell-specific metadata, leaving unspecified options unset so
/// the downstream generator can apply its defaults.
fn parse_windows_meta(meta: &ParseNestedMeta, windows: &mut WindowsAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("module_name") {
            windows.module_name = Some(lit_str(&nested, "module_name")?.value());
            return Ok(());
        }
        if nested.path.is_ident("export_aliases") {
            windows.export_aliases = parse_string_array(&nested, "export_aliases")?;
            return Ok(());
        }
        if nested.path.is_ident("include_common_parameters") {
            windows.include_common_parameters = Some(parse_bool(&nested)?);
            return Ok(());
        }
        if nested.path.is_ident("split_subcommands") {
            windows.split_subcommands = Some(parse_bool(&nested)?);
            return Ok(());
        }
        if nested.path.is_ident("help_info_uri") {
            windows.help_info_uri = Some(lit_str(&nested, "help_info_uri")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })
}
/// Reads the explicit documentation value-type override for a field.
fn parse_value_meta(meta: &ParseNestedMeta, out: &mut DocFieldAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("type") {
            out.value_type = Some(lit_str(&nested, "type")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })
}
/// Associates a deprecation notice with the localized note that explains it.
fn parse_deprecated_meta(meta: &ParseNestedMeta, out: &mut DocFieldAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("note_id") {
            out.deprecated_note_id = Some(lit_str(&nested, "note_id")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })
}
/// Records an explicit environment-variable name for generated field docs.
fn parse_env_meta(meta: &ParseNestedMeta, out: &mut DocFieldAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("name") {
            out.env_name = Some(lit_str(&nested, "name")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })
}
/// Records a dotted configuration-file key path for generated field docs.
fn parse_file_meta(meta: &ParseNestedMeta, out: &mut DocFieldAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("key_path") {
            out.file_key_path = Some(lit_str(&nested, "key_path")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })
}
/// Captures CLI naming and help-visibility overrides used by both parsing and
/// documentation generation.
fn parse_cli_meta(meta: &ParseNestedMeta, out: &mut FieldAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("long") {
            out.cli_long = Some(lit_str(&nested, "long")?.value());
            return Ok(());
        }
        if nested.path.is_ident("short") {
            out.cli_short = Some(lit_char(&nested, "short")?);
            return Ok(());
        }
        if nested.path.is_ident("value_name") {
            out.doc.cli_value_name = Some(lit_str(&nested, "value_name")?.value());
            return Ok(());
        }
        if nested.path.is_ident("hide_in_help") {
            out.doc.cli_hide_in_help = parse_bool(&nested)?;
            return Ok(());
        }
        discard_unknown(&nested)
    })
}
/// Parses a Rust array expression containing only string literals.
///
/// Preserving source order keeps precedence lists and `PowerShell` aliases
/// deterministic; other expression forms produce an error at the bad value.
fn parse_string_array(meta: &ParseNestedMeta, key: &str) -> syn::Result<Vec<String>> {
    let expr_value = meta.value()?.parse::<syn::Expr>()?;
    let expr_array = match expr_value {
        syn::Expr::Array(array) => array,
        other => {
            return Err(syn::Error::new(
                other.span(),
                format!("{key} must be an array of string literals"),
            ));
        }
    };
    let mut values = Vec::new();
    for element in expr_array.elems {
        match element {
            syn::Expr::Lit(lit_expr) => {
                if let syn::Lit::Str(s) = lit_expr.lit {
                    values.push(s.value());
                } else {
                    return Err(syn::Error::new(
                        lit_expr.span(),
                        format!("{key} must contain string literals"),
                    ));
                }
            }
            other => {
                return Err(syn::Error::new(
                    other.span(),
                    format!("{key} must contain string literals"),
                ));
            }
        }
    }
    Ok(values)
}
/// Parses a bare boolean attribute as `true`, or its explicit literal value.
fn parse_bool(meta: &ParseNestedMeta) -> syn::Result<bool> {
    if meta.input.peek(Token![=]) {
        return Ok(meta.value()?.parse::<LitBool>()?.value);
    }
    Ok(true)
}
