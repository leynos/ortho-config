//! Crate path resolution for dependency aliasing support.
//!
//! Converts the optional `#[ortho_config(crate = "...")]` attribute value
//! into a `TokenStream` that replaces hardcoded `ortho_config::` paths in
//! generated code.

use proc_macro2::TokenStream;
use quote::quote;

/// Resolve the crate path from the parsed struct attribute.
///
/// Returns `ortho_config` when no override is present, allowing the
/// generated code to reference types through an aliased dependency name.
///
/// # Examples
///
/// ```rust,ignore
/// let default = resolve(None);
/// assert_eq!(default.to_string(), "ortho_config");
///
/// let path: syn::Path = syn::parse_str("my_alias").unwrap();
/// let aliased = resolve(Some(&path));
/// assert_eq!(aliased.to_string(), "my_alias");
/// ```
pub(crate) fn resolve(crate_path: Option<&syn::Path>) -> TokenStream {
    crate_path.map_or_else(|| quote! { ortho_config }, |path| quote! { #path })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_ortho_config() {
        let tokens = resolve(None);
        assert_eq!(tokens.to_string(), "ortho_config");
    }

    #[test]
    fn uses_custom_path() {
        let path: syn::Path = syn::parse_str("my_alias").expect("valid path");
        let tokens = resolve(Some(&path));
        assert_eq!(tokens.to_string(), "my_alias");
    }

    #[test]
    fn supports_nested_path() {
        let path: syn::Path = syn::parse_str("my_ns::ortho_config").expect("valid path");
        let tokens = resolve(Some(&path));
        assert_eq!(tokens.to_string(), "my_ns :: ortho_config");
    }
}
