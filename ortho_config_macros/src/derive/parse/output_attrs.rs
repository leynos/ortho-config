//! Parsing and validation for the `output(...)` struct-level attribute.
//!
//! Kept in its own module (like `behaviour_attrs` and `renderer_attrs`) so
//! `doc_attrs` stays under the repository's 400-line ceiling.
//!
//! The group is strict: unknown keys, wrong literal types, a blank class
//! meaning, a malformed class name, and repeated assignment to the same leaf
//! are hard errors with spans. Separate groups may contribute distinct leaves,
//! so `output(supports_json = true)` followed by
//! `output(stdout = "data")` is accepted while a repeated `stdout` is rejected.
//!
//! The leaf parsers are the shared helpers in [`super::renderer_values`]
//! (`parse_bool` and `parse_token`); this module adds only the group structure
//! and the `exit_class(...)` entry grammar. `parse_token` lives beside the
//! renderer attribute because that group introduced it, but it validates one
//! string token against an accepted table and carries no renderer semantics.

use syn::meta::ParseNestedMeta;
use syn::parse::{Parse as _, ParseStream};
use syn::spanned::Spanned;
use syn::{Expr, Lit, MetaNameValue, Token};

use super::doc_types::{ExitClassAttr, OutputAttrs};
use super::renderer_values::{parse_bool, parse_token};

/// Accepted `stdout` purpose tokens.
const STDOUT_VALUES: &[&str] = &["data", "path", "summary", "none"];

/// Parses the nested keys of one `output(...)` declaration.
///
/// Declaring `output()` with no members is accepted and leaves every leaf
/// unset: an explicitly empty block makes no capability claim.
pub(crate) fn parse_output_meta(
    meta: &ParseNestedMeta,
    output: &mut OutputAttrs,
) -> syn::Result<()> {
    // `output()` supplies no members. Syn's nested-attribute loop treats an
    // empty group as a missing attribute, so the empty case is consumed here
    // before delegating: an explicitly empty block claims nothing.
    if group_is_empty(meta.input)? {
        let _empty;
        syn::parenthesized!(_empty in meta.input);
        return Ok(());
    }
    meta.parse_nested_meta(|nested| {
        let Some(ident) = nested.path.get_ident() else {
            return unknown_output_key(&nested);
        };
        match ident.to_string().as_str() {
            "supports_json" => {
                let value = parse_bool(&nested)?;
                set_once(&mut output.supports_json, value, &nested, "supports_json")
            }
            "stdout" => {
                let value = parse_token(&nested, "stdout", STDOUT_VALUES, "stdout")?;
                set_once(&mut output.stdout, value, &nested, "stdout")
            }
            "stderr_diagnostics" => {
                let value = parse_bool(&nested)?;
                set_once(
                    &mut output.stderr_diagnostics,
                    value,
                    &nested,
                    "stderr_diagnostics",
                )
            }
            "exit_class" => {
                let class = parse_exit_class(&nested)?;
                output.exit_classes.push(class);
                reject_duplicate_class(&output.exit_classes, &nested)
            }
            _ => unknown_output_key(&nested),
        }
    })
}

/// Reports whether the input starts with an empty parenthesized group.
///
/// The check inspects a fork, so a non-empty group is left untouched for the
/// caller's `parse_nested_meta` to consume normally.
fn group_is_empty(input: ParseStream<'_>) -> syn::Result<bool> {
    if !input.peek(syn::token::Paren) {
        return Ok(false);
    }
    let fork = input.fork();
    let content;
    syn::parenthesized!(content in fork);
    Ok(content.is_empty())
}

/// Reports an unrecognized key inside an `output(...)` declaration.
fn unknown_output_key(nested: &ParseNestedMeta) -> syn::Result<()> {
    let path = &nested.path;
    let key = quote::quote!(#path).to_string();
    Err(syn::Error::new(
        path.span(),
        format!(
            "unknown output attribute `{key}`; expected `supports_json`, `stdout`, \
             `stderr_diagnostics`, or `exit_class`"
        ),
    ))
}

/// Parses one `exit_class(name = "...", meaning = "...")` entry.
///
/// The entry names its members explicitly so neither argument is positional:
/// a bare string would leave "which string is the meaning" ambiguous.
fn parse_exit_class(nested: &ParseNestedMeta) -> syn::Result<ExitClassAttr> {
    let content;
    syn::parenthesized!(content in nested.input);
    let members: syn::punctuated::Punctuated<MetaNameValue, Token![,]> =
        content.parse_terminated(MetaNameValue::parse, Token![,])?;

    let mut name = None;
    let mut meaning = None;
    for member in members {
        let Some(ident) = member.path.get_ident() else {
            return Err(unknown_class_member(&member.path));
        };
        let key = ident.to_string();
        let literal = string_literal(&member, &key)?;
        match key.as_str() {
            "name" => set_once_lit(&mut name, literal, &key)?,
            "meaning" => set_once_lit(&mut meaning, literal, &key)?,
            _ => return Err(unknown_class_member(&member.path)),
        }
    }

    let name_lit = name.ok_or_else(|| {
        syn::Error::new(
            nested.path.span(),
            "exit_class requires `name = \"...\"`; it is the stable class identifier",
        )
    })?;
    let meaning_lit = meaning.ok_or_else(|| {
        syn::Error::new(
            nested.path.span(),
            "exit_class requires `meaning = \"...\"`; it explains when the class applies",
        )
    })?;

    let class_name = name_lit.value();
    validate_class_name(&class_name, &name_lit)?;
    let class_meaning = meaning_lit.value();
    if class_meaning.trim().is_empty() {
        return Err(syn::Error::new(
            meaning_lit.span(),
            "exit class meaning must not be blank; it is the explanation agents read",
        ));
    }

    Ok(ExitClassAttr {
        name: class_name,
        meaning: class_meaning,
    })
}

/// Reports an unrecognized member of an `exit_class(...)` entry.
fn unknown_class_member(path: &syn::Path) -> syn::Error {
    let key = quote::quote!(#path).to_string();
    syn::Error::new(
        path.span(),
        format!("unknown exit class member `{key}`; expected `name` or `meaning`"),
    )
}

/// Extracts a string literal from one `key = "value"` class member.
fn string_literal<'a>(member: &'a MetaNameValue, key: &str) -> syn::Result<&'a syn::LitStr> {
    match &member.value {
        Expr::Lit(expr) => match &expr.lit {
            Lit::Str(literal) => Ok(literal),
            _ => Err(syn::Error::new(
                expr.span(),
                format!("exit class `{key}` must be a string literal"),
            )),
        },
        other => Err(syn::Error::new(
            other.span(),
            format!("exit class `{key}` must be a string literal"),
        )),
    }
}

/// Stores a class member literal, rejecting a second assignment.
fn set_once_lit(slot: &mut Option<syn::LitStr>, value: &syn::LitStr, key: &str) -> syn::Result<()> {
    if slot.is_some() {
        return Err(syn::Error::new(
            value.span(),
            format!("duplicate exit class member `{key}`; assign it once per class"),
        ));
    }
    *slot = Some(value.clone());
    Ok(())
}

/// Validates the stable-token grammar `[a-z][a-z0-9_]*` for a class name.
///
/// The grammar is what makes a name usable as a non-localized protocol
/// identifier: no case folding, spacing, or punctuation can drift with a
/// locale or a human-language setting.
fn validate_class_name(name: &str, literal: &syn::LitStr) -> syn::Result<()> {
    let mut characters = name.chars();
    let is_valid = characters.next().is_some_and(|first| {
        first.is_ascii_lowercase()
            && characters
                .all(|rest| rest.is_ascii_lowercase() || rest.is_ascii_digit() || rest == '_')
    });
    if is_valid {
        return Ok(());
    }
    Err(syn::Error::new(
        literal.span(),
        format!(
            "invalid exit class name '{name}'; expected a stable token matching \
             [a-z][a-z0-9_]*"
        ),
    ))
}

/// Rejects a repeated exit class name within one command's declaration.
///
/// Comparison is exact: names are protocol identifiers, so the parser never
/// normalizes case or spacing to make a near-duplicate look unique.
fn reject_duplicate_class(classes: &[ExitClassAttr], nested: &ParseNestedMeta) -> syn::Result<()> {
    let Some(latest) = classes.last() else {
        return Ok(());
    };
    let repeats = classes
        .iter()
        .rev()
        .skip(1)
        .any(|earlier| earlier.name == latest.name);
    if repeats {
        return Err(syn::Error::new(
            nested.path.span(),
            format!(
                "duplicate exit class '{}'; class names must be unique within one command",
                latest.name
            ),
        ));
    }
    Ok(())
}

/// Stores a leaf value, rejecting a second assignment to the same leaf.
///
/// Repeated groups are safe to merge because distinct leaves may come from
/// separate `output(...)` declarations, while the same leaf twice is ambiguous.
fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    nested: &ParseNestedMeta,
    key: &str,
) -> syn::Result<()> {
    if slot.is_some() {
        return Err(syn::Error::new(
            nested.path.span(),
            format!("duplicate output policy `{key}`; assign it once per command"),
        ));
    }
    *slot = Some(value);
    Ok(())
}
