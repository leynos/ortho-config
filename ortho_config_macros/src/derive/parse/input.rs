//! Input parsing for the `OrthoConfig` derive macro.
//!
//! This module gathers the struct identifier, fields, and relevant attribute
//! metadata in one pass so macro expansion can fail fast with useful errors.

use syn::{Attribute, Data, DeriveInput, Fields};

use super::{
    FieldAttrs, StructAttrs, option_inner, parse_field_attrs, parse_struct_attrs,
    serde_deserialized_field_key, serde_field_is_flattened, serde_rename_rules, vec_inner,
};

/// Gathers information from the user-provided struct.
///
/// The helper collects the struct identifier, its fields, and all
/// attribute metadata in one pass. Returning these components together
/// keeps the `derive` implementation simple and validates invalid input
/// eagerly so expansion can fail fast.
///
/// Each field's `serialized_key` is the key Serde reads when deserializing the
/// final configuration type. Generated layer structs serialize under that key,
/// so the key model is resolved once here rather than recomputed by each
/// generator. A field that cannot express a single configuration key, such as
/// one marked `#[serde(flatten)]`, is rejected separately by
/// [`reject_incompatible_flatten`].
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
    let fields = named_fields(input)?;
    let field_attrs = parse_field_attrs_with_keys(&fields, &input.attrs)?;
    validate_project_root_field(&fields, &field_attrs, &struct_attrs)?;
    Ok((ident, fields, struct_attrs, field_attrs))
}

/// Parses every field and caches the key the configuration type reads.
///
/// The container `rename_all` rule is resolved once, so per-field renames and
/// container rules produce the same key model for every generator.
fn parse_field_attrs_with_keys(
    fields: &[syn::Field],
    container_attrs: &[Attribute],
) -> Result<Vec<FieldAttrs>, syn::Error> {
    let rename_all = serde_rename_rules(container_attrs)?.deserialize;
    fields
        .iter()
        .map(|field| {
            let mut attrs = parse_field_attrs(field)?;
            attrs.serialized_key = serde_deserialized_field_key(field, rename_all)?;
            Ok(attrs)
        })
        .collect()
}

/// Rejects `#[serde(flatten)]` where the derive would generate a layer key.
///
/// A flattened field has no key of its own: Serde reads its children from the
/// enclosing object. That is compatible with the required-value schema, where
/// the field is only inspected (or explicitly composed with `nested`), but not
/// with a generated layer that must name one CLI option, default value, or
/// collection entry. Those cases would emit a key the final type never reads,
/// so they are rejected at the derive site instead.
///
/// # Examples
///
/// A flattened child marked `#[ortho_config(skip_cli, nested)]` with no default
/// or merge strategy is accepted. The same field without `skip_cli` is
/// rejected because the generated CLI would parse the whole child as one
/// argument.
pub(crate) fn reject_incompatible_flatten(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> syn::Result<()> {
    for (field, attrs) in fields.iter().zip(field_attrs) {
        if !serde_field_is_flattened(&field.attrs)? {
            continue;
        }
        let Some((reason, remedy)) = flatten_conflict(field, attrs) else {
            continue;
        };
        let name = field
            .ident
            .as_ref()
            .map_or_else(String::new, |ident| ident.to_string());
        return Err(syn::Error::new_spanned(
            field,
            format!(
                "`#[serde(flatten)]` is unsupported on field `{name}` because {reason}; {remedy}, \
                 or remove the flatten"
            ),
        ));
    }
    Ok(())
}

/// Names why a generated layer cannot represent a flattened field and the
/// remedy for that reason, if any.
fn flatten_conflict(
    field: &syn::Field,
    attrs: &FieldAttrs,
) -> Option<(&'static str, &'static str)> {
    if attrs.cli.is_option() {
        Some((
            "the generated CLI would parse it as a single argument",
            "add `#[ortho_config(skip_cli)]`",
        ))
    } else if attrs.default.is_some() || attrs.inferred_clap_default.is_some() {
        Some((
            "its default would be layered under a key the configuration never reads",
            "remove the declared default",
        ))
    } else if attrs.merge_strategy.is_some() {
        Some((
            "a collection merge strategy needs one collection key",
            "remove the merge strategy",
        ))
    } else if vec_inner(&field.ty).is_some() {
        // `Vec` fields append by default even without an explicit strategy.
        Some((
            "a Vec field is accumulated under a single collection key",
            "exclude the collection from generated layers",
        ))
    } else {
        None
    }
}

/// Returns the struct's named fields, rejecting enums, unions and tuple structs.
fn named_fields(input: &DeriveInput) -> Result<Vec<syn::Field>, syn::Error> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input.ident.clone(),
            "OrthoConfig can only be derived for structs",
        ));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new_spanned(
            data.struct_token,
            "OrthoConfig requires named fields",
        ));
    };
    Ok(named.named.iter().cloned().collect())
}

/// Accepts a path type whose final segment is `PathBuf`, including qualified paths.
///
/// # Examples
///
/// `std::path::PathBuf` is accepted because its final path segment is
/// `PathBuf`; `Path` is rejected.
fn is_path_buf(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "PathBuf")
}

/// Checks that a configured project-root selector names a usable CLI path field.
///
/// Missing, skipped, or non-path fields are rejected at the derive site so the
/// generated discovery code never has to guess which field supplies the root.
///
/// # Examples
///
/// A configured `root: PathBuf` field that remains available to the CLI is
/// accepted; a `root: String` field produces a derive diagnostic.
fn validate_project_root_field(
    fields: &[syn::Field],
    attrs: &[FieldAttrs],
    struct_attrs: &StructAttrs,
) -> syn::Result<()> {
    let Some(discovery) = &struct_attrs.discovery else {
        return Ok(());
    };
    let Some(name) = &discovery.project_root_from else {
        return Ok(());
    };
    let Some((field, field_attrs)) = fields
        .iter()
        .zip(attrs)
        .find(|(field, _)| field.ident.as_ref().is_some_and(|ident| ident == name))
    else {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("`project_root_from` must name a CLI PathBuf field; `{name}` was not found"),
        ));
    };
    let valid_type = is_path_buf(&field.ty) || option_inner(&field.ty).is_some_and(is_path_buf);
    if field_attrs.cli.is_skipped() || !valid_type {
        return Err(syn::Error::new_spanned(
            field,
            "`project_root_from` must name a non-skipped `PathBuf` or `Option<PathBuf>` field",
        ));
    }
    Ok(())
}
