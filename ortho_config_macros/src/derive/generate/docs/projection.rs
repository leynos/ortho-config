//! Generated locals that project metadata into a subcommand namespace.
//!
//! A subcommand merge reads environment variables from
//! `<PREFIX>_CMDS_<NAME>_` and file keys from `cmds.<name>`. Generated
//! documentation emits those names only when the metadata describes a mounted
//! subcommand; root metadata keeps the prefix-and-field name.
//!
//! Locals are emitted only when a generated environment or file name needs
//! projection. An empty configuration would otherwise leave unused bindings in
//! the generated function body, which a consumer compiling with `-D warnings`
//! must not inherit.

use proc_macro2::TokenStream;
use quote::quote;

/// Token fragments that project metadata into a subcommand namespace.
pub(super) struct ProjectionTokens {
    /// Environment prefix base with at most one trailing underscore removed.
    prefix_base: String,
    /// Whether an environment projection local is required.
    needs_env: bool,
    /// Whether a file projection local is required.
    needs_file: bool,
}

impl ProjectionTokens {
    /// Captures the struct prefix and the projections the field set needs.
    pub(super) fn new(prefix: Option<&str>, needs_env: bool, needs_file: bool) -> Self {
        let prefix_base = prefix
            .map(|value| value.strip_suffix('_').unwrap_or(value).to_owned())
            .unwrap_or_default();
        Self {
            prefix_base,
            needs_env,
            needs_file,
        }
    }

    /// Names the helper parameter, marking it unused when nothing projects.
    pub(super) fn subcommand_parameter(&self) -> TokenStream {
        if self.needs_env || self.needs_file {
            quote! { subcommand }
        } else {
            quote! { _subcommand }
        }
    }

    /// Emits the environment-prefix local when one is needed.
    pub(super) fn env_prefix_local(&self) -> TokenStream {
        if !self.needs_env {
            return TokenStream::new();
        }
        let prefix = syn::LitStr::new(&self.prefix_base, proc_macro2::Span::call_site());
        quote! {
            let doc_env_prefix = subcommand.map(|name| {
                let env_key = name.replace('-', "_").to_ascii_uppercase();
                let prefix = #prefix;
                format!("{prefix}_CMDS_{env_key}_")
            });
        }
    }

    /// Emits the file-prefix local when one is needed.
    pub(super) fn file_prefix_local(&self) -> TokenStream {
        if !self.needs_file {
            return TokenStream::new();
        }
        quote! {
            let doc_file_prefix = subcommand.map(|name| format!("cmds.{name}."));
        }
    }
}
