//! Parsing and validation for the `behaviour(...)` struct-level attribute.
//!
//! Kept in its own module (like `serde_attrs` and `clap_attrs`) so `doc_attrs`
//! stays under the repository's 400-line ceiling.

use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;

use super::doc_types::BehaviourAttrs;
use super::literals::lit_str;

/// Validates the `--flag` grammar shared by `bypass` and `dry_run`.
///
/// A declared flag must match `--[a-z0-9]+(-[a-z0-9]+)*` so consumers can rely
/// on a single unambiguous wire shape (see ADR-008).
fn validate_flag_grammar(value: &str, span: proc_macro2::Span) -> syn::Result<()> {
    let body = value.strip_prefix("--").unwrap_or("");
    let valid = !body.is_empty()
        && body.split('-').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
    if !valid {
        return Err(syn::Error::new(
            span,
            format!(
                "invalid flag '{value}'; flags must match --[a-z0-9]+(-[a-z0-9]+)* (for example \"--force\" or \"--no-input\")"
            ),
        ));
    }
    Ok(())
}

/// Parses the nested keys of a `behaviour(...)` declaration.
///
/// Each nested key is applied by [`apply_behaviour_field`]; once the group has
/// parsed, [`validate_behaviour`] checks the merged attribute state. Unknown
/// nested keys and invalid values are hard errors (with spans) so a misspelt
/// key or value cannot be silently swallowed by `discard_unknown`.
pub(crate) fn parse_behaviour_meta(
    meta: &ParseNestedMeta,
    behaviour: &mut BehaviourAttrs,
) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| apply_behaviour_field(&nested, behaviour))?;
    validate_behaviour(meta, behaviour)
}

/// Applies one nested key of a `behaviour(...)` declaration.
///
/// Unknown nested keys and invalid values are hard errors (with spans) so a
/// misspelt key or value cannot be silently swallowed by `discard_unknown`.
fn apply_behaviour_field(
    nested: &ParseNestedMeta,
    behaviour: &mut BehaviourAttrs,
) -> syn::Result<()> {
    let key = nested
        .path
        .get_ident()
        .map(ToString::to_string)
        .unwrap_or_default();
    match key.as_str() {
        "interaction" => apply_interaction(nested, behaviour),
        "mutation" => apply_mutation(nested, behaviour),
        "bypass" => apply_bypass(nested, behaviour),
        "dry_run" => apply_dry_run(nested, behaviour),
        _ => {
            let path = &nested.path;
            let flag = quote::quote!(#path).to_string();
            Err(syn::Error::new(
                path.span(),
                format!("unknown behaviour attribute `{flag}`"),
            ))
        }
    }
}

/// Applies the `interaction` nested key.
fn apply_interaction(nested: &ParseNestedMeta, behaviour: &mut BehaviourAttrs) -> syn::Result<()> {
    let value_lit = lit_str(nested, "interaction")?;
    let value = value_lit.value();
    let validated = match value.as_str() {
        "non_interactive" => "non_interactive",
        "interactive" => "interactive",
        other => {
            return Err(syn::Error::new(
                value_lit.span(),
                format!(
                    "unknown interaction '{other}'; expected \"non_interactive\" or \"interactive\""
                ),
            ));
        }
    };
    behaviour.interaction = Some(validated.to_owned());
    Ok(())
}

/// Applies the `mutation` nested key.
fn apply_mutation(nested: &ParseNestedMeta, behaviour: &mut BehaviourAttrs) -> syn::Result<()> {
    let value_lit = lit_str(nested, "mutation")?;
    let value = value_lit.value();
    match value.as_str() {
        "read_only" | "write" | "delete" | "submit" => {
            behaviour.mutation = Some(value);
            Ok(())
        }
        other => Err(syn::Error::new(
            value_lit.span(),
            format!(
                "unknown mutation '{other}'; expected one of \"read_only\", \"write\", \"delete\", or \"submit\""
            ),
        )),
    }
}

/// Applies the `bypass` nested key.
fn apply_bypass(nested: &ParseNestedMeta, behaviour: &mut BehaviourAttrs) -> syn::Result<()> {
    let value_lit = lit_str(nested, "bypass")?;
    validate_flag_grammar(&value_lit.value(), value_lit.span())?;
    behaviour.bypass = Some(value_lit.value());
    Ok(())
}

/// Applies the `dry_run` nested key.
fn apply_dry_run(nested: &ParseNestedMeta, behaviour: &mut BehaviourAttrs) -> syn::Result<()> {
    let value_lit = lit_str(nested, "dry_run")?;
    validate_flag_grammar(&value_lit.value(), value_lit.span())?;
    behaviour.dry_run = Some(value_lit.value());
    Ok(())
}

/// Validates the merged `BehaviourAttrs` state after a group parses.
///
/// A bypass flag exists to skip a confirmation prompt; a command declared
/// non-interactive never prompts, so the combination is contradictory
/// (see ADR-008). The check runs against the merged state so a
/// non-interactive declaration and a bypass split across repeated
/// `behaviour(...)` groups is still rejected.
fn validate_behaviour(meta: &ParseNestedMeta, behaviour: &BehaviourAttrs) -> syn::Result<()> {
    if behaviour.interaction.as_deref() != Some("non_interactive") || behaviour.bypass.is_none() {
        return Ok(());
    }
    Err(syn::Error::new(
        meta.path.span(),
        "contradictory behaviour: interaction = \"non_interactive\" combined with bypass; a non-interactive command never prompts, so there is nothing to bypass",
    ))
}
