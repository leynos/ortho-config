//! Parsing and validation for the `renderer(...)` struct-level attribute.
//!
//! Kept in its own module (like `behaviour_attrs` and `serde_attrs`) so
//! `doc_attrs` stays under the repository's 400-line ceiling.
//!
//! The group is strict: unknown nested keys, invalid enum tokens, wrong literal
//! types, and repeated assignment to the same leaf are hard errors with spans.
//! Separate groups may contribute distinct leaves, so
//! `renderer(human(colour = "never"), human(width = 80))` is accepted while a
//! repeated `colour` is rejected.

use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;

use super::doc_types::{RendererAttrs, RendererRecordAttrs};
use super::renderer_values::{
    ACCESSIBILITY_VALUES, CLOSED_STDIN_VALUES, INHERITANCE_VALUES, PLAIN_VALUES, STREAM_VALUES,
    TOGGLE_VALUES, TTY_VALUES, parse_bool, parse_field_entry, parse_token, parse_width,
    reject_duplicate_fields,
};

/// Parses the nested keys of a `renderer(...)` declaration.
pub(crate) fn parse_renderer_meta(
    meta: &ParseNestedMeta,
    renderer: &mut RendererAttrs,
) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("human") {
            return parse_record_group(&nested, renderer, RecordRole::Human);
        }
        if nested.path.is_ident("machine") {
            return parse_record_group(&nested, renderer, RecordRole::Machine);
        }
        let path = &nested.path;
        let key = quote::quote!(#path).to_string();
        Err(syn::Error::new(
            path.span(),
            format!("unknown renderer attribute `{key}`; expected `human` or `machine`"),
        ))
    })
}

/// Which renderer record a `renderer(...)` group addresses.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RecordRole {
    /// The human renderer.
    Human,
    /// The machine renderer.
    Machine,
}

/// Parses one `human(...)` or `machine(...)` group into its record.
///
/// Repeating a group merges into the existing record so distinct leaves may be
/// contributed across groups; a leaf already set is a duplicate-assignment
/// error rather than a silent overwrite.
fn parse_record_group(
    nested: &ParseNestedMeta,
    renderer: &mut RendererAttrs,
    role: RecordRole,
) -> syn::Result<()> {
    let mut record = match role {
        RecordRole::Human => {
            let declared = renderer.human_declared;
            renderer.human_declared = true;
            debug_assert!(
                !declared || renderer.human.is_some(),
                "a declared human group must have a record"
            );
            renderer.human.take().unwrap_or_default()
        }
        RecordRole::Machine => {
            let declared = renderer.machine_declared;
            renderer.machine_declared = true;
            debug_assert!(
                !declared || renderer.machine.is_some(),
                "a declared machine group must have a record"
            );
            renderer.machine.take().unwrap_or_default()
        }
    };

    let parsed = nested.parse_nested_meta(|key| apply_record_field(&key, &mut record));

    match role {
        RecordRole::Human => renderer.human = Some(record),
        RecordRole::Machine => renderer.machine = Some(record),
    }
    parsed
}

/// Applies one nested key inside a renderer record group.
fn apply_record_field(
    nested: &ParseNestedMeta,
    record: &mut RendererRecordAttrs,
) -> syn::Result<()> {
    let Some(ident) = nested.path.get_ident() else {
        return unknown_record_key(nested);
    };
    let key = ident.to_string();
    match key.as_str() {
        "supported" => set_once(&mut record.supported, parse_bool(nested)?, nested, &key),
        "tty" => set_token(nested, &key, &mut record.tty, TTY_VALUES),
        "closed_stdin" => set_token(nested, &key, &mut record.closed_stdin, CLOSED_STDIN_VALUES),
        "colour" => set_token(nested, &key, &mut record.colour, TOGGLE_VALUES),
        "emoji" => set_token(nested, &key, &mut record.emoji, TOGGLE_VALUES),
        "progress" => set_token(nested, &key, &mut record.progress, TOGGLE_VALUES),
        "pager" => set_token(nested, &key, &mut record.pager, TOGGLE_VALUES),
        "width" => set_once(&mut record.width, parse_width(nested)?, nested, &key),
        "accessibility" => set_token(
            nested,
            &key,
            &mut record.accessibility,
            ACCESSIBILITY_VALUES,
        ),
        "plain" => set_token(nested, &key, &mut record.plain, PLAIN_VALUES),
        "stdout" => set_token(nested, &key, &mut record.stdout, STREAM_VALUES),
        "stderr" => set_token(nested, &key, &mut record.stderr, STREAM_VALUES),
        "progress_inheritance" => set_token(
            nested,
            &key,
            &mut record.progress_inheritance,
            INHERITANCE_VALUES,
        ),
        "subprocess_inheritance" => set_token(
            nested,
            &key,
            &mut record.subprocess_inheritance,
            INHERITANCE_VALUES,
        ),
        "field" => {
            let entry = parse_field_entry(nested)?;
            record.fields.push(entry);
            reject_duplicate_fields(&record.fields, nested)
        }
        _ => unknown_record_key(nested),
    }
}

/// Validates one token leaf and stores it, rejecting a repeated assignment.
fn set_token(
    nested: &ParseNestedMeta,
    key: &str,
    slot: &mut Option<String>,
    values: &[&str],
) -> syn::Result<()> {
    // The diagnostic label is the key itself: every token leaf is named by the
    // key that declares it, so a separate argument would only repeat it.
    let value = parse_token(nested, key, values, key)?;
    set_once(slot, value, nested, key)
}

/// Reports an unrecognized key inside a renderer record with its span.
fn unknown_record_key(nested: &ParseNestedMeta) -> syn::Result<()> {
    let path = &nested.path;
    let key = quote::quote!(#path).to_string();
    Err(syn::Error::new(
        path.span(),
        format!("unknown renderer policy `{key}`"),
    ))
}

/// Stores a leaf value, rejecting a second assignment to the same leaf.
///
/// The duplicate check is what makes a repeated group safe to merge: distinct
/// leaves may come from separate groups, but the same leaf twice is ambiguous.
fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    nested: &ParseNestedMeta,
    key: &str,
) -> syn::Result<()> {
    if slot.is_some() {
        return Err(syn::Error::new(
            nested.path.span(),
            format!("duplicate renderer policy `{key}`; assign it once per renderer"),
        ));
    }
    *slot = Some(value);
    Ok(())
}
