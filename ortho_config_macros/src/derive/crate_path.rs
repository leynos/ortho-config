//! Crate path resolution for dependency aliasing support.
//!
//! Converts the optional `#[ortho_config(crate = "...")]` attribute value
//! into a `TokenStream` that replaces hardcoded `ortho_config::` paths in
//! generated code.

use proc_macro2::TokenStream;
use quote::quote;

/// Resolve the crate path from the parsed struct attribute.
///
/// Defaults to `ortho_config` when no override is present. When the user
/// specifies `#[ortho_config(crate = "...")]`, the returned tokens
/// reference types through the aliased dependency name instead.
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
    //! Unit tests for crate path resolution with default and custom paths.

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::default(None, "ortho_config")]
    #[case::custom(Some("my_alias"), "my_alias")]
    #[case::nested(Some("my_ns::ortho_config"), "my_ns :: ortho_config")]
    fn resolve_produces_expected_tokens(#[case] input: Option<&str>, #[case] expected: &str) {
        let parsed = input.map(|s| syn::parse_str::<syn::Path>(s).expect("valid path"));
        let tokens = resolve(parsed.as_ref());
        assert_eq!(tokens.to_string(), expected);
    }
}
