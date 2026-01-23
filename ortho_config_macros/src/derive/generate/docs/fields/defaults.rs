//! Default value generators for field documentation metadata.

use super::super::AppName;

/// Generates the default field ID for help or `long_help`.
pub(super) fn default_field_id(app_name: &AppName, field: &str, suffix: &str) -> String {
    format!("{}.fields.{field}.{suffix}", &**app_name)
}

/// Generates the default environment variable name from prefix and field.
///
/// Inserts an underscore between prefix and field when joining (unless the prefix
/// already ends with '_' or the field starts with '_') to produce names like
/// `APP_URL` instead of `APPURL`.
pub(super) fn default_env_name(prefix: Option<&str>, field: &str) -> String {
    let mut name = String::new();
    if let Some(prefix_value) = prefix {
        name.push_str(prefix_value);
        // Insert underscore separator if needed
        let prefix_ends_with_underscore = prefix_value.ends_with('_');
        let field_starts_with_underscore = field.starts_with('_');
        if !prefix_ends_with_underscore && !field_starts_with_underscore {
            name.push('_');
        }
    }
    name.push_str(field);
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_env_name_inserts_underscore() {
        assert_eq!(default_env_name(Some("APP"), "url"), "APP_URL");
        assert_eq!(default_env_name(Some("APP"), "log_level"), "APP_LOG_LEVEL");
    }

    #[test]
    fn default_env_name_no_double_underscore_when_prefix_ends_with_underscore() {
        assert_eq!(default_env_name(Some("APP_"), "url"), "APP_URL");
    }

    #[test]
    fn default_env_name_no_double_underscore_when_field_starts_with_underscore() {
        assert_eq!(default_env_name(Some("APP"), "_internal"), "APP_INTERNAL");
    }

    #[test]
    fn default_env_name_no_prefix() {
        assert_eq!(default_env_name(None, "url"), "URL");
    }
}
