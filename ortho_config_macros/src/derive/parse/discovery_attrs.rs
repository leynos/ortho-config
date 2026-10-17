//! Discovery-attribute models shared by the derive parser and loader emitter.

use syn::{Lit, meta::ParseNestedMeta};

/// Parsed fields from `#[ortho_config(discovery(...))]`.
#[derive(Default, Clone)]
pub(crate) struct DiscoveryAttrs {
    /// Application identity used to derive conventional discovery locations.
    pub app_name: Option<String>,
    /// Single environment selector for an explicitly named configuration file.
    pub env_var: Option<String>,
    /// Ordered environment selectors; mutually exclusive with `env_var`.
    pub env_vars: Vec<String>,
    /// Discovery mode used when the caller names a configuration file.
    pub explicit_mode: Option<String>,
    /// Discovery mode used while searching conventional locations.
    pub automatic_mode: Option<String>,
    /// Scope precedence used to order discovered configuration sources.
    pub scope_order: Vec<String>,
    /// CLI `PathBuf` field used as the project-root search anchor.
    pub project_root_from: Option<String>,
    /// Explicit name for the standard configuration file.
    pub config_file_name: Option<String>,
    /// Explicit name for the user-level dotfile.
    pub dotfile_name: Option<String>,
    /// Explicit name for the project-level configuration file.
    pub project_file_name: Option<String>,
    /// Long option used to supply an explicit configuration path.
    pub config_cli_long: Option<String>,
    /// Short option used to supply an explicit configuration path.
    pub config_cli_short: Option<char>,
    /// Whether the generated configuration-path option appears in help.
    pub config_cli_visible: Option<bool>,
}

impl DiscoveryAttrs {
    /// Reports whether discovery policy differs from filename and app defaults.
    ///
    /// The loader emitter uses this to decide whether explicit policy metadata
    /// needs to be generated.
    ///
    /// # Examples
    ///
    /// Default attributes return `false`; setting `scope_order` makes the
    /// method return `true` so the emitter preserves the explicit policy.
    pub(crate) const fn uses_policy(&self) -> bool {
        !self.env_vars.is_empty()
            || self.explicit_mode.is_some()
            || self.automatic_mode.is_some()
            || !self.scope_order.is_empty()
            || self.project_root_from.is_some()
    }
}

/// Collection strategy requested by a field attribute.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MergeStrategy {
    /// Adds higher-precedence sequence values after values already accumulated.
    Append,
    /// Replaces the accumulated collection with the higher-precedence value.
    Replace,
    /// Merges map entries by key, preserving untouched lower-layer entries.
    Keyed,
}

impl MergeStrategy {
    /// Parses the supported spelling and points invalid values back to the attribute.
    ///
    /// # Examples
    ///
    /// `"keyed"` produces `MergeStrategy::Keyed`; `"concat"` returns a
    /// diagnostic attached to the supplied span.
    pub(crate) fn parse(value: &str, span: proc_macro2::Span) -> Result<Self, syn::Error> {
        match value {
            "append" => Ok(Self::Append),
            "replace" => Ok(Self::Replace),
            "keyed" => Ok(Self::Keyed),
            _ => Err(syn::Error::new(
                span,
                format!(
                    "unknown merge_strategy '{value}'; expected one of \"append\", \"replace\", or \"keyed\""
                ),
            )),
        }
    }
}

/// Parses a string prefix and appends `_` to non-empty values when needed.
///
/// A non-string literal is rejected at its own span so diagnostics identify
/// the invalid attribute value.
///
/// # Examples
///
/// The attribute value `"APP"` becomes `"APP_"`; an empty string stays empty.
pub(crate) fn parse_prefix(meta: &ParseNestedMeta) -> syn::Result<String> {
    match meta.value()?.parse::<Lit>()? {
        Lit::Str(string) => {
            let mut value = string.value();
            if !value.is_empty() && !value.ends_with('_') {
                value.push('_');
            }
            Ok(value)
        }
        other => Err(syn::Error::new(other.span(), "prefix must be a string")),
    }
}
