//! Ensures configuration helpers are accessible via re-exports.

use ortho_config::{figment::Figment, uncased::UncasedStr};

#[test]
fn reexports_are_public() {
    let _figment = Figment::default();
    let _uncased = UncasedStr::new("key");
    #[cfg(feature = "toml")]
    {
        use ortho_config::toml;
        let _ = toml::Value::String("ok".to_string());
    }
    #[cfg(feature = "yaml")]
    {
        use ortho_config::serde_yaml;
        let _ = serde_yaml::Value::Null;
    }
    #[cfg(feature = "json5")]
    {
        use ortho_config::figment::providers::Format as _;
        use ortho_config::{figment_json5::Json5, json5};
        let _ = Json5::file("dummy.json5");
        let _: Result<serde_json::Value, _> = json5::from_str("{}");
    }
    #[cfg(any(unix, target_os = "redox"))]
    {
        use ortho_config::xdg::BaseDirectories;
        let _ = BaseDirectories::with_prefix("myapp");
    }
}
