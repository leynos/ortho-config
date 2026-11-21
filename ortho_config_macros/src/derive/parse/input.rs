//! Input parsing for the `OrthoConfig` derive macro.
//!
//! This module gathers the struct identifier, fields, and relevant attribute
//! metadata in one pass so macro expansion can fail fast with useful errors.

use syn::{Data, DeriveInput, Fields};

use super::{FieldAttrs, StructAttrs, option_inner, parse_field_attrs, parse_struct_attrs};

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
    let fields = named_fields(input)?;
    let field_attrs = fields
        .iter()
        .map(parse_field_attrs)
        .collect::<Result<Vec<_>, _>>()?;
    validate_project_root_field(&fields, &field_attrs, &struct_attrs)?;
    Ok((ident, fields, struct_attrs, field_attrs))
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
