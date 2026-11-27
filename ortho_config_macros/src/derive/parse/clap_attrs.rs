//! Parsing helpers for clap field and variant attributes.
//!
//! These helpers extract metadata from `#[arg(...)]` and `#[clap(...)]`
//! attributes without taking a dependency on clap itself.

use syn::ext::IdentExt;

use super::clap_rename::ClapRenameAll;

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
        // Consume the value of an unrelated key so the enclosing nested-meta
        // loop stays positioned on the next comma. clap accepts both
        // `key = expr` and `key(expr)`; an unconsumed parenthesized group
        // makes the loop report a spurious missing comma.
        if meta.input.peek(syn::Token![=]) {
            let value = meta.value()?;
            let _: syn::Expr = value.parse()?;
        } else if meta.input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in meta.input);
            content.parse::<proc_macro2::TokenStream>()?;
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

/// Reads the explicit `id = "..."` override for a field, when one is written.
///
/// Only [`clap_arg_id_or_field_name`] should need this: callers that want the
/// id clap will actually record must go through the resolver, so the fallback
/// is applied in one place.
pub(crate) fn clap_arg_id(field: &syn::Field) -> syn::Result<Option<String>> {
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

/// Reads a clap key's argument from either `= expr` or `(expr)`.
///
/// clap accepts both spellings, and its method-call form accepts a trailing
/// comma. The parenthesized form always consumes its group, then reports the
/// content only when it parses as exactly one expression, so an unrelated
/// multi-argument method is ignored rather than turned into a derive error.
fn clap_argument(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<Option<syn::Expr>> {
    if meta.input.peek(syn::Token![=]) {
        return Ok(Some(meta.value()?.parse()?));
    }
    if meta.input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in meta.input);
        let arguments =
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated(&content)?;
        if arguments.len() != 1 {
            return Ok(None);
        }
        return Ok(arguments.into_iter().next());
    }
    Ok(None)
}

/// Consume the value or parenthesized tokens of an unrelated clap key so the
/// nested-meta parser can continue without interpreting clap's full grammar.
fn consume_unknown_meta(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<()> {
    clap_argument(meta).map(drop)
}

/// Reads the string literal a clap key argument spells, when it is literal.
fn literal_string(expression: Option<syn::Expr>) -> Option<syn::LitStr> {
    match expression {
        Some(syn::Expr::Lit(literal)) => match literal.lit {
            syn::Lit::Str(value) => Some(value),
            _ => None,
        },
        _ => None,
    }
}

/// Reads the container's `#[command(rename_all = "...")]` casing rule.
///
/// A non-literal or unknown casing is ignored: clap owns the attribute's
/// grammar, and the derive must not reject a declaration it cannot resolve.
pub(crate) fn clap_rename_all(attrs: &[syn::Attribute]) -> syn::Result<ClapRenameAll> {
    let mut rename_all = ClapRenameAll::default();
    for attr in attrs.iter().filter(|attr| is_clap_command_attribute(attr)) {
        let syn::Meta::List(_) = &attr.meta else {
            continue;
        };
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("rename_all") {
                return consume_unknown_meta(&meta);
            }
            if let Some(value) = literal_string(clap_argument(&meta)?)
                && let Some(parsed) = ClapRenameAll::parse(&value.value())
            {
                rename_all = parsed;
            }
            Ok(())
        })?;
    }
    Ok(rename_all)
}

/// Reads an explicit clap command name from a struct's `#[command(name)]`.
///
/// The subcommand merge names its environment and file namespace from the
/// payload's `CommandFactory` command name, so documentation metadata uses the
/// same declaration where the author supplied one. A non-literal value is
/// ignored rather than rejected: clap accepts expressions, and the derive must
/// not reject a valid clap attribute it cannot resolve statically.
pub(crate) fn clap_command_name(attrs: &[syn::Attribute]) -> syn::Result<Option<String>> {
    let mut name = None;
    for attr in attrs.iter().filter(|attr| is_clap_command_attribute(attr)) {
        let syn::Meta::List(_) = &attr.meta else {
            continue;
        };
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("name") {
                return consume_unknown_meta(&meta);
            }
            if let Some(value) = literal_string(clap_argument(&meta)?) {
                name = Some(value.value());
            }
            Ok(())
        })?;
    }
    Ok(name)
}

/// Parse a clap command `name = "..."` override from an enum variant.
///
/// Both `name = "..."` and `name("...")` are accepted. Unlike the struct-level
/// command name, a variant name must resolve statically: the selector derives
/// its runtime label and identifiers from it, so a non-literal value is
/// rejected rather than silently replaced by a guess.
pub(crate) fn clap_variant_name(variant: &syn::Variant) -> syn::Result<Option<syn::LitStr>> {
    let mut name = None;
    for attr in variant
        .attrs
        .iter()
        .filter(|attr| is_clap_command_attribute(attr))
    {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("name") {
                return consume_unknown_meta(&meta);
            }
            let expression = clap_argument(&meta)?.ok_or_else(|| {
                syn::Error::new_spanned(&meta.path, "clap command `name` requires a value")
            })?;
            let literal = literal_string(Some(expression)).ok_or_else(|| {
                syn::Error::new_spanned(&meta.path, "clap command `name` must be a string literal")
            })?;
            name = Some(literal);
            Ok(())
        })?;
    }
    Ok(name)
}

/// Record one nested meta item for [`clap_field_env`].
///
/// An `env` key either carries a literal name or is bare; every other key is
/// consumed without inspection. Kept out of the caller's `parse_nested_meta`
/// closure so the `if`/`else` sits at the depth of its own function body rather
/// than nesting inside two loops.
fn consume_env_meta(
    meta: &syn::meta::ParseNestedMeta<'_>,
    env: &mut Option<syn::LitStr>,
    bare_env: &mut bool,
) -> syn::Result<()> {
    if !meta.path.is_ident("env") {
        return consume_unknown_meta(meta);
    }
    if meta.input.peek(syn::Token![=]) {
        let value = meta.value()?;
        *env = Some(value.parse()?);
    } else {
        *bare_env = true;
    }
    Ok(())
}

/// Read the environment variable name a clap field binds, if any.
///
/// Two forms reach `clap`:
///
/// - `#[arg(env = "NAME")]` binds `NAME` directly.
/// - A bare `#[arg(env)]` takes the name from the argument's identifier, which
///   `clap_derive` renders with `ToShoutySnakeCase` (`item.rs`'s
///   `DEFAULT_ENV_CASING` is `ScreamingSnake`, applied to `self.name`). A
///   packed field binding
///   `#[arg(id = "custom", env)]` uses the *assigned* id instead of the field
///   ident, so the inferred name follows whichever of the two clap will use.
///
/// Used by the profile flag collision check (decision D6): a user field that
/// claims the generated `<PREFIX>PROFILE` binding is a compile-time error on an
/// opted-in struct. Returning `None` for a form whose name cannot be resolved
/// locally keeps that check from rejecting a name it cannot prove.
pub(crate) fn clap_field_env(field: &syn::Field) -> syn::Result<Option<String>> {
    let mut env: Option<syn::LitStr> = None;
    let mut bare_env = false;
    for attr in field.attrs.iter().filter(|attr| is_clap_attribute(attr)) {
        let syn::Meta::List(list) = &attr.meta else {
            continue;
        };
        list.parse_nested_meta(|meta| consume_env_meta(&meta, &mut env, &mut bare_env))?;
    }
    if let Some(lit) = env {
        return Ok(Some(lit.value()));
    }
    if !bare_env {
        return Ok(None);
    }
    // A bare `env` takes its name from the argument's identifier: the assigned
    // `id` when present, otherwise the field ident. Both use the same casing
    // rule as `clap_derive`'s `ScreamingSnake` default.
    let source = clap_arg_id(field)?.map_or_else(
        || field.ident.as_ref().map(|ident| ident.unraw().to_string()),
        Some,
    );
    Ok(source.map(|name| heck::ToShoutySnakeCase::to_shouty_snake_case(name.as_str())))
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
