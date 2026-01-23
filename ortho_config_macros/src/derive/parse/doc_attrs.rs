//! Documentation-related attribute parsing.

use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;
use syn::{LitBool, Token};

use super::doc_types::{
    DocExampleAttr, DocFieldAttrs, DocLinkAttr, DocNoteAttr, DocStructAttrs, HeadingOverrides,
    PrecedenceAttrs, WindowsAttrs,
};
use super::literals::{lit_char, lit_str};
use super::{FieldAttrs, discard_unknown};

pub(crate) fn apply_struct_doc_attr(
    meta: &ParseNestedMeta,
    out: &mut DocStructAttrs,
) -> syn::Result<bool> {
    let Some(ident) = meta.path.get_ident() else {
        return Ok(false);
    };
    let key = ident.to_string();
    match key.as_str() {
        "about_id" => {
            out.about_id = Some(lit_str(meta, "about_id")?.value());
            Ok(true)
        }
        "synopsis_id" => {
            out.synopsis_id = Some(lit_str(meta, "synopsis_id")?.value());
            Ok(true)
        }
        "bin_name" => {
            out.bin_name = Some(lit_str(meta, "bin_name")?.value());
            Ok(true)
        }
        "headings" => {
            let mut headings = out.headings.clone();
            parse_headings_meta(meta, &mut headings)?;
            out.headings = headings;
            Ok(true)
        }
        "precedence" => {
            let mut precedence = out.precedence.take().unwrap_or_default();
            parse_precedence_meta(meta, &mut precedence)?;
            out.precedence = Some(precedence);
            Ok(true)
        }
        "example" => {
            out.examples.push(parse_example_meta(meta)?);
            Ok(true)
        }
        "link" => {
            out.links.push(parse_link_meta(meta)?);
            Ok(true)
        }
        "note" => {
            out.notes.push(parse_note_meta(meta)?);
            Ok(true)
        }
        "windows" => {
            let mut windows = out.windows.take().unwrap_or_default();
            parse_windows_meta(meta, &mut windows)?;
            out.windows = Some(windows);
            Ok(true)
        }
        _ => Ok(false),
    }
}
pub(crate) fn apply_field_doc_attr(
    meta: &ParseNestedMeta,
    out: &mut FieldAttrs,
) -> syn::Result<bool> {
    let Some(ident) = meta.path.get_ident() else {
        return Ok(false);
    };
    let key = ident.to_string();
    match key.as_str() {
        "help_id" => {
            out.doc.help_id = Some(lit_str(meta, "help_id")?.value());
            Ok(true)
        }
        "long_help_id" => {
            out.doc.long_help_id = Some(lit_str(meta, "long_help_id")?.value());
            Ok(true)
        }
        "value" => {
            parse_value_meta(meta, &mut out.doc)?;
            Ok(true)
        }
        "deprecated" => {
            parse_deprecated_meta(meta, &mut out.doc)?;
            Ok(true)
        }
        "required" => {
            out.doc.required = Some(parse_bool(meta)?);
            Ok(true)
        }
        "env" => {
            parse_env_meta(meta, &mut out.doc)?;
            Ok(true)
        }
        "file" => {
            parse_file_meta(meta, &mut out.doc)?;
            Ok(true)
        }
        "cli" => {
            parse_cli_meta(meta, out)?;
            Ok(true)
        }
        "example" => {
            out.doc.examples.push(parse_example_meta(meta)?);
            Ok(true)
        }
        "link" => {
            out.doc.links.push(parse_link_meta(meta)?);
            Ok(true)
        }
        "note" => {
            out.doc.notes.push(parse_note_meta(meta)?);
            Ok(true)
        }
        _ => Ok(false),
    }
}
fn parse_headings_meta(meta: &ParseNestedMeta, headings: &mut HeadingOverrides) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        let Some(ident) = nested.path.get_ident() else {
            return discard_unknown(&nested);
        };
        let key = ident.to_string();
        let value = lit_str(&nested, &key)?.value();
        match key.as_str() {
            "name" => headings.name = Some(value),
            "synopsis" => headings.synopsis = Some(value),
            "description" => headings.description = Some(value),
            "options" => headings.options = Some(value),
            "environment" => headings.environment = Some(value),
            "files" => headings.files = Some(value),
            "precedence" => headings.precedence = Some(value),
            "exit_status" => headings.exit_status = Some(value),
            "examples" => headings.examples = Some(value),
            "see_also" => headings.see_also = Some(value),
            _ => return discard_unknown(&nested),
        }
        Ok(())
    })
}
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
fn parse_example_meta(meta: &ParseNestedMeta) -> syn::Result<DocExampleAttr> {
    let mut title_id = None;
    let mut code = None;
    let mut body_id = None;
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("title_id") {
            title_id = Some(lit_str(&nested, "title_id")?.value());
            return Ok(());
        }
        if nested.path.is_ident("code") {
            code = Some(lit_str(&nested, "code")?.value());
            return Ok(());
        }
        if nested.path.is_ident("body_id") {
            body_id = Some(lit_str(&nested, "body_id")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })?;
    let code_value =
        code.ok_or_else(|| syn::Error::new(meta.path.span(), "example requires code"))?;
    Ok(DocExampleAttr {
        title_id,
        code: code_value,
        body_id,
    })
}
fn parse_link_meta(meta: &ParseNestedMeta) -> syn::Result<DocLinkAttr> {
    let mut text_id = None;
    let mut uri = None;
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("text_id") {
            text_id = Some(lit_str(&nested, "text_id")?.value());
            return Ok(());
        }
        if nested.path.is_ident("uri") {
            uri = Some(lit_str(&nested, "uri")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })?;
    let uri_value = uri.ok_or_else(|| syn::Error::new(meta.path.span(), "link requires uri"))?;
    Ok(DocLinkAttr {
        text_id,
        uri: uri_value,
    })
}
fn parse_note_meta(meta: &ParseNestedMeta) -> syn::Result<DocNoteAttr> {
    let mut text_id = None;
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("text_id") {
            text_id = Some(lit_str(&nested, "text_id")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })?;
    let text_id_value =
        text_id.ok_or_else(|| syn::Error::new(meta.path.span(), "note requires text_id"))?;
    Ok(DocNoteAttr {
        text_id: text_id_value,
    })
}
fn parse_value_meta(meta: &ParseNestedMeta, out: &mut DocFieldAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("type") {
            out.value_type = Some(lit_str(&nested, "type")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })
}
fn parse_deprecated_meta(meta: &ParseNestedMeta, out: &mut DocFieldAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("note_id") {
            out.deprecated_note_id = Some(lit_str(&nested, "note_id")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })
}
fn parse_env_meta(meta: &ParseNestedMeta, out: &mut DocFieldAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("name") {
            out.env_name = Some(lit_str(&nested, "name")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })
}
fn parse_file_meta(meta: &ParseNestedMeta, out: &mut DocFieldAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("key_path") {
            out.file_key_path = Some(lit_str(&nested, "key_path")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })
}
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
fn parse_bool(meta: &ParseNestedMeta) -> syn::Result<bool> {
    if meta.input.peek(Token![=]) {
        return Ok(meta.value()?.parse::<LitBool>()?.value);
    }
    Ok(true)
}
