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
pub(super) fn generate_non_object_guard(
    config_ident: &syn::Ident,
    krate: &TokenStream,
) -> TokenStream {
    quote! {
        let provenance_label = match provenance {
            #krate::MergeProvenance::Defaults => "defaults",
            #krate::MergeProvenance::File => "file",
            #krate::MergeProvenance::Environment => "environment",
            #krate::MergeProvenance::Cli => "CLI",
            _ => "unknown",
        };
        let value_kind = match other {
            #krate::serde_json::Value::Null => "null",
            #krate::serde_json::Value::Bool(_) => "a boolean",
            #krate::serde_json::Value::Number(_) => "a number",
            #krate::serde_json::Value::String(_) => "a string",
            #krate::serde_json::Value::Array(_) => "an array",
            #krate::serde_json::Value::Object(_) => "an object",
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
        return Err(std::sync::Arc::new(#krate::OrthoError::merge(
            #krate::figment::Error::from(message),
        )));
    }
}
