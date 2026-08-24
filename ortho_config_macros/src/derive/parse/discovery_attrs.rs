//! Discovery-attribute models shared by the derive parser and loader emitter.

use syn::{Lit, meta::ParseNestedMeta};

/// Parsed fields from `#[ortho_config(discovery(...))]`.
#[derive(Default, Clone)]
pub(crate) struct DiscoveryAttrs {
    pub app_name: Option<String>,
    pub env_var: Option<String>,
    pub env_vars: Vec<String>,
    pub explicit_mode: Option<String>,
    pub automatic_mode: Option<String>,
    pub scope_order: Vec<String>,
    pub project_root_from: Option<String>,
    pub config_file_name: Option<String>,
    pub dotfile_name: Option<String>,
    pub project_file_name: Option<String>,
    pub config_cli_long: Option<String>,
    pub config_cli_short: Option<char>,
    pub config_cli_visible: Option<bool>,
}

impl DiscoveryAttrs {
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
    Append,
    Replace,
    Keyed,
}

impl MergeStrategy {
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

/// Parse the struct-level prefix, normalizing its separator.
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
