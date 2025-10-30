//! Ensures configuration helpers are accessible via re-exports.
use ortho_config::{figment::Figment, uncased::UncasedStr};

#[test]
fn reexports_are_public() {
    let _figment = Figment::default();
    let _uncased = UncasedStr::new("key");
    #[cfg(feature = "toml")]
    {
        use ortho_config::toml;
        let _ = toml::Value::String("ok".to_owned());
    }
    #[cfg(feature = "yaml")]
    {
        use ortho_config::serde_saphyr;
        let value = serde_saphyr::from_str::<serde_json::Value>("key: value")
            .expect("serde-saphyr should parse YAML");
        let actual = value
            .get("key")
            .expect("expected key entry in parsed YAML")
            .as_str()
            .expect("expected YAML key to deserialize as string");
        assert_eq!(actual, "value");
    }
    #[cfg(feature = "json5")]
    {
        use ortho_config::figment::providers::Format as _;
        use ortho_config::{figment_json5::Json5, json5};
        let _ = Json5::file("dummy.json5");
        assert!(json5::from_str::<serde_json::Value>("{}").is_ok());
    }
    #[cfg(any(unix, target_os = "redox"))]
    {
        use ortho_config::xdg::BaseDirectories;
        let _ = BaseDirectories::with_prefix("myapp");
    }
}
