//! Default value generators for field documentation metadata.

use super::super::AppName;

/// Generates the default field ID for help or `long_help`.
pub(super) fn default_field_id(app_name: &AppName, field: &str, suffix: &str) -> String {
    format!("{}.fields.{field}.{suffix}", &**app_name)
}

/// Builds the environment suffix a generated name uses for `key`.
///
/// The provider strips the prefix, uppercases the remainder, splits it on
/// `__`, and lowercases each part. A suffix is offered only when replaying
/// those transforms reproduces the key exactly. An uppercase letter fails
/// because the final lowercase changes it, and a literal dot, an embedded
/// `__`, whitespace, `=`, NUL, or a non-ASCII byte cannot be reproduced at all.
/// A key the provider cannot read back receives no generated environment
/// variable, because advertising one would send callers to a variable that
/// populates a different key.
///
/// # Examples
///
/// ```ignore
/// use super::generated_env_suffix;
///
/// assert_eq!(generated_env_suffix("log_level").as_deref(), Some("LOG_LEVEL"));
/// assert_eq!(generated_env_suffix("verify-mode").as_deref(), Some("VERIFY-MODE"));
/// ```
pub(super) fn generated_env_suffix(key: &str) -> Option<String> {
    let expressible = !key.is_empty()
        && !key.contains('.')
        && !key.contains("__")
        && key.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        });
    expressible.then(|| key.to_ascii_uppercase())
}

/// Appends a generated suffix to the configured prefix.
///
/// A separator is inserted only when neither side already supplies one, so
/// `APP` and `LOG_LEVEL` become `APP_LOG_LEVEL`, and `APP` and `_INTERNAL`
/// become `APP_INTERNAL`.
pub(super) fn join_env_prefix(prefix: Option<&str>, suffix: &str) -> String {
    let Some(configured) = prefix else {
        return suffix.to_owned();
    };
    let mut name = String::with_capacity(configured.len() + suffix.len() + 1);
    name.push_str(configured);
    if !configured.ends_with('_') && !suffix.starts_with('_') {
        name.push('_');
    }
    name.push_str(suffix);
    name
}

#[cfg(test)]
mod tests {
    //! Unit tests for generated environment names and field identifiers.

    use super::{AppName, default_field_id, generated_env_suffix, join_env_prefix};

    /// A plain identifier keeps its underscore-separated suffix.
    #[test]
    fn generated_env_suffix_uppercases_identifiers() {
        assert_eq!(generated_env_suffix("url").as_deref(), Some("URL"));
        assert_eq!(
            generated_env_suffix("log_level").as_deref(),
            Some("LOG_LEVEL")
        );
    }

    /// A camel-case key cannot survive the provider's lowercase step.
    #[test]
    fn generated_env_suffix_rejects_uppercase_letters() {
        assert_eq!(generated_env_suffix("logLevel"), None);
    }

    /// A kebab-case key keeps its hyphen so the provider reads it back.
    #[test]
    fn generated_env_suffix_preserves_kebab_separators() {
        assert_eq!(
            generated_env_suffix("verify-mode").as_deref(),
            Some("VERIFY-MODE")
        );
    }

    /// A dotted or nested key has no single-variable spelling.
    #[test]
    fn generated_env_suffix_rejects_nested_keys() {
        assert_eq!(generated_env_suffix("server.port"), None);
        assert_eq!(generated_env_suffix("a__b"), None);
        assert_eq!(generated_env_suffix(" padded"), None);
        assert_eq!(generated_env_suffix(""), None);
    }

    /// The prefix separator is inserted only when neither side supplies one.
    #[test]
    fn join_env_prefix_inserts_one_separator() {
        assert_eq!(join_env_prefix(Some("APP"), "URL"), "APP_URL");
        assert_eq!(join_env_prefix(Some("APP_"), "URL"), "APP_URL");
        assert_eq!(join_env_prefix(Some("APP"), "_INTERNAL"), "APP_INTERNAL");
        assert_eq!(join_env_prefix(None, "URL"), "URL");
    }

    /// The default field identifier keeps the historical dotted shape.
    #[test]
    fn default_field_id_uses_the_application_name() {
        let app = AppName::new(String::from("demo"));
        assert_eq!(
            default_field_id(&app, "host", "help"),
            "demo.fields.host.help"
        );
    }
}
