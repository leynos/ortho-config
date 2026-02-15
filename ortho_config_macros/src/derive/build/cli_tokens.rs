//! CLI struct token construction for the `OrthoConfig` derive macro.
//!
//! Assembles the generated CLI struct fields, conditionally adding a
//! `config_path` flag when the user's struct does not define one.

use crate::CliFieldInfo;
use crate::derive::build::{
    CliFieldMetadata, build_cli_field_metadata, build_cli_struct_fields, build_config_flag_field,
};
use crate::derive::parse::{SerdeRenameAll, clap_arg_id, serde_serialized_field_key};

/// Result of building CLI struct tokens.
pub(crate) struct CliStructBuildResult {
    /// The generated CLI struct fields.
    pub fields: Vec<proc_macro2::TokenStream>,
    /// Metadata for each field used in `CliValueExtractor` generation.
    ///
    /// This is derived from the input configuration struct fields only (not any
    /// generated CLI-only fields).
    pub field_info: Vec<CliFieldInfo>,
    /// Metadata for each field used in documentation generation.
    pub metadata: Vec<CliFieldMetadata>,
}

/// Construct a [`CliFieldInfo`] record for a single struct field.
fn build_cli_field_info(
    field: &syn::Field,
    attrs: &crate::derive::parse::FieldAttrs,
    serde_rename_all: Option<SerdeRenameAll>,
) -> syn::Result<CliFieldInfo> {
    let name = field
        .ident
        .clone()
        .ok_or_else(|| syn::Error::new_spanned(field, "unnamed fields are not supported"))?;
    let field_name = name.to_string();
    let arg_id = clap_arg_id(field)?.unwrap_or_else(|| field_name.clone());
    let serialized_key = serde_serialized_field_key(field, serde_rename_all)?;
    Ok(CliFieldInfo {
        serialized_key,
        arg_id,
        is_default_as_absent: attrs.cli_default_as_absent,
    })
}

/// Build CLI struct field tokens, conditionally adding a generated
/// `config_path` field.
///
/// If no user-defined `config_path` field exists, generates a config flag field
/// based on discovery attributes from `struct_attrs`.
///
/// # Arguments
///
/// - `fields`: Struct fields used to generate CLI tokens.
/// - `field_attrs`: Per-field attributes controlling CLI generation.
/// - `struct_attrs`: Struct-level attributes including discovery settings.
/// - `serde_rename_all`: Optional serde rename-all strategy.
///
/// # Errors
///
/// Returns an error if CLI flag collisions are detected.
pub(crate) fn build_cli_struct_tokens(
    fields: &[syn::Field],
    field_attrs: &[crate::derive::parse::FieldAttrs],
    struct_attrs: &crate::derive::parse::StructAttrs,
    serde_rename_all: Option<SerdeRenameAll>,
) -> syn::Result<CliStructBuildResult> {
    let mut cli_struct = build_cli_struct_fields(fields, field_attrs)?;
    if !cli_struct.field_names.contains("config_path") {
        let config_field = build_config_flag_field(
            struct_attrs,
            &cli_struct.used_shorts,
            &cli_struct.used_longs,
            &cli_struct.field_names,
        )?;
        cli_struct.fields.push(config_field);
    }

    let metadata = build_cli_field_metadata(fields, field_attrs)?;

    let field_info: Vec<CliFieldInfo> = fields
        .iter()
        .zip(field_attrs)
        .filter(|(_, attrs)| !attrs.skip_cli)
        .map(|(field, attrs)| build_cli_field_info(field, attrs, serde_rename_all))
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(CliStructBuildResult {
        fields: cli_struct.fields,
        field_info,
        metadata,
    })
}
