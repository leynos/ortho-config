//! Validation helpers for field documentation metadata.

use std::collections::HashMap;

use syn::Ident;

/// Validates that an environment variable name is well-formed.
pub(super) fn validate_env_name(field: &Ident, env_name: &str) -> syn::Result<()> {
    if env_name.is_empty() {
        return Err(syn::Error::new_spanned(
            field,
            "environment variable names must be non-empty",
        ));
    }
    if env_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        field,
        format!(
            "environment variable '{env_name}' must contain only ASCII alphanumeric characters or '_'",
        ),
    ))
}

/// Validates that a file key path is well-formed.
pub(super) fn validate_file_key(field: &Ident, key_path: &str) -> syn::Result<()> {
    if key_path.is_empty() {
        return Err(syn::Error::new_spanned(
            field,
            "file key paths must be non-empty",
        ));
    }
    for segment in key_path.split('.') {
        if segment.is_empty() {
            return Err(syn::Error::new_spanned(
                field,
                format!("file key path '{key_path}' must not contain empty segments"),
            ));
        }
        let valid = segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
        if !valid {
            return Err(syn::Error::new_spanned(
                field,
                format!(
                    "file key path '{key_path}' must contain only ASCII alphanumeric characters, '_' or '-'",
                ),
            ));
        }
    }
    Ok(())
}

/// Ensures a key is unique, tracking seen keys and their spans.
pub(super) fn ensure_unique(
    kind: &str,
    field: &Ident,
    key: &str,
    seen: &mut HashMap<String, proc_macro2::Span>,
) -> syn::Result<()> {
    if let Some(existing) = seen.get(key) {
        let mut err =
            syn::Error::new_spanned(field, format!("duplicate {kind} identifier '{key}'"));
        err.combine(syn::Error::new(*existing, "first defined here"));
        return Err(err);
    }
    seen.insert(key.to_owned(), field.span());
    Ok(())
}
