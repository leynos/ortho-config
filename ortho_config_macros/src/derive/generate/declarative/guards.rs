//! Guard generation for declarative merge.
//!
//! Generates error handling tokens for non-object JSON values encountered
//! during the merge process.

use proc_macro2::TokenStream;
use quote::quote;

/// Generate the non-object guard error handling.
///
/// Produces tokens that emit a descriptive error when a merge layer contains
/// a non-object JSON value, identifying the provenance and value kind.
pub(super) fn generate_non_object_guard(config_ident: &syn::Ident) -> TokenStream {
    quote! {
        let provenance_label = match provenance {
            ortho_config::MergeProvenance::Defaults => "defaults",
            ortho_config::MergeProvenance::File => "file",
            ortho_config::MergeProvenance::Environment => "environment",
            ortho_config::MergeProvenance::Cli => "CLI",
            _ => "unknown",
        };
        let value_kind = match other {
            ortho_config::serde_json::Value::Null => "null",
            ortho_config::serde_json::Value::Bool(_) => "a boolean",
            ortho_config::serde_json::Value::Number(_) => "a number",
            ortho_config::serde_json::Value::String(_) => "a string",
            ortho_config::serde_json::Value::Array(_) => "an array",
            ortho_config::serde_json::Value::Object(_) => "an object",
        };
        let mut message = format!(
            concat!(
                "Declarative merge for ",
                stringify!(#config_ident),
                " expects JSON objects but the ",
                "{provenance_label} layer supplied {value_kind}. "
            ),
            provenance_label = provenance_label,
            value_kind = value_kind,
        );
        if let Some(path) = path {
            message.push_str("Source: ");
            message.push_str(path.as_str());
            message.push_str(". ");
        }
        message.push_str("Non-object layers would overwrite accumulated state.");
        return Err(std::sync::Arc::new(ortho_config::OrthoError::merge(
            ortho_config::figment::Error::from(message),
        )));
    }
}
