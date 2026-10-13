//! Parsing helpers for clap field and variant attributes.
//!
//! These helpers extract metadata from `#[arg(...)]` and `#[clap(...)]`
//! attributes without taking a dependency on clap itself.

use syn::ext::IdentExt;

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

pub(crate) fn clap_arg_id_from_attribute(
    attr: &syn::Attribute,
    existing_id: &mut Option<syn::LitStr>,
) -> syn::Result<()> {
    let syn::Meta::List(list) = &attr.meta else {
        return Ok(());
    };

    list.parse_nested_meta(|meta| parse_id_from_meta(&meta, existing_id))
}

/// Reads the explicit `id = "..."` override for a field, when one is written.
///
/// Only [`clap_arg_id_or_field_name`] should need this: callers that want the
/// id clap will actually record must go through the resolver, so the fallback
/// is applied in one place.
fn clap_arg_id(field: &syn::Field) -> syn::Result<Option<String>> {
    let mut arg_id: Option<syn::LitStr> = None;
    for attr in field.attrs.iter().filter(|attr| is_clap_attribute(attr)) {
        clap_arg_id_from_attribute(attr, &mut arg_id)?;
    }
    Ok(arg_id.map(|lit| lit.value()))
}

/// Resolves the clap argument id for a field, deriving it when unset.
///
/// `clap_derive` builds the id for a field with no explicit `id = "..."` from
/// `ident.unraw().to_string()`: it strips the `r#` of a raw identifier and only
/// kebab-cases the *long flag*, never the id. Deriving the fallback from
/// `Ident::to_string()` instead would keep the `r#`, so `r#type` would emit the
/// id `r#type` while clap records `type`.
///
/// Both the generated `clap` attribute and every consumer that looks the
/// argument up again in `ArgMatches` must agree on that spelling, so the
/// fallback lives here rather than being re-derived at each call site.
pub(crate) fn clap_arg_id_or_field_name(field: &syn::Field) -> syn::Result<String> {
    if let Some(explicit) = clap_arg_id(field)? {
        return Ok(explicit);
    }
    let ident = field
        .ident
        .as_ref()
        .ok_or_else(|| syn::Error::new_spanned(field, "unnamed fields are not supported"))?;
    Ok(ident.unraw().to_string())
}

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

/// A clap field marker requested from a command attribute.
///
/// Drives the shared attribute traversal in [`clap_field_matches_marker`] for
/// subcommand selectors and flattened argument groups.
#[derive(Clone, Copy)]
enum ClapFieldMarker {
    /// Marks the field that selects a subcommand.
    Subcommand,
    /// Marks a field whose arguments are flattened into the parent command.
    Flatten,
}

impl ClapFieldMarker {
    /// The clap key denoting this marker inside `#[command(...)]` and
    /// `#[clap(...)]`.
    const fn ident(self) -> &'static str {
        match self {
            Self::Subcommand => "subcommand",
            Self::Flatten => "flatten",
        }
    }
}

/// Shared attribute traversal for [`clap_field_is_subcommand`] and
/// [`clap_field_is_flattened`].
///
/// Scans every `#[command(...)]`/`#[clap(...)]` attribute on the field, flags
/// when the requested [`ClapFieldMarker`] key appears, and routes all other
/// nested metadata through [`consume_unknown_meta`] so unrelated clap keys
/// parse without error.
fn clap_field_matches_marker(field: &syn::Field, marker: ClapFieldMarker) -> syn::Result<bool> {
    let mut matches = false;
    for attr in field
        .attrs
        .iter()
        .filter(|attr| is_clap_command_attribute(attr))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(marker.ident()) {
                matches = true;
                return Ok(());
            }
            consume_unknown_meta(&meta)
        })?;
    }
    Ok(matches)
}

/// Detect whether a struct field is a clap subcommand selector.
pub(crate) fn clap_field_is_subcommand(field: &syn::Field) -> syn::Result<bool> {
    clap_field_matches_marker(field, ClapFieldMarker::Subcommand)
}

/// Detect whether a struct field is a flattened clap args group.
///
/// Mirrors [`clap_field_is_subcommand`] so flattened groups are recognised
/// from both `#[command(flatten)]` and `#[clap(flatten)]`. Flattened fields
/// surface their arguments under the parent command at runtime but cannot be
/// enumerated by the parent derive, so they are excluded from generated
/// `ARG_IDS` (Decision D-12) and are expected to carry their own
/// `OrthoConfigLocalization` implementation.
pub(crate) fn clap_field_is_flattened(field: &syn::Field) -> syn::Result<bool> {
    clap_field_matches_marker(field, ClapFieldMarker::Flatten)
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
