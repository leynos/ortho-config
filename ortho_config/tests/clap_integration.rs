//! Tests covering CLI integration and error handling.

use ortho_config::{OrthoConfig, OrthoError};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct TestConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    other: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct OptionConfig {
    maybe: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct RequiredConfig {
    sample_value: String,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct ConflictConfig {
    second: Option<String>,
    sample: Option<String>,
}

#[rstest]
fn parses_kebab_case_flags() {
    let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "hello", "--other", "val"])
        .expect("load");
    assert_eq!(cfg.sample_value.as_deref(), Some("hello"));
    assert_eq!(cfg.other.as_deref(), Some("val"));
}

#[rstest]
fn short_flags_work() {
    let cfg = TestConfig::load_from_iter(["prog", "-s", "hi", "-o", "val"]).expect("load");
    assert_eq!(cfg.sample_value.as_deref(), Some("hi"));
    assert_eq!(cfg.other.as_deref(), Some("val"));
}

#[rstest]
fn cli_only_source() {
    let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "only", "--other", "x"])
        .expect("load");
    assert_eq!(cfg.sample_value.as_deref(), Some("only"));
    assert_eq!(cfg.other.as_deref(), Some("x"));
}

#[rstest]
fn cli_overrides_other_sources() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "sample_value = \"file\"\nother = \"f\"")?;
        j.set_env("SAMPLE_VALUE", "env");
        j.set_env("OTHER", "e");
        let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "cli", "--other", "cli2"])
            .expect("load");
        assert_eq!(cfg.sample_value.as_deref(), Some("cli"));
        assert_eq!(cfg.other.as_deref(), Some("cli2"));
        Ok(())
    });
}

#[rstest]
fn cli_combines_with_file() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "other = \"file\"")?;
        let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "cli", "--other", "cli2"])
            .expect("load");
        assert_eq!(cfg.sample_value.as_deref(), Some("cli"));
        // CLI argument should override file value
        assert_eq!(cfg.other.as_deref(), Some("cli2"));
        Ok(())
    });
}

#[rstest]
fn invalid_cli_input_maps_error() {
    let err = TestConfig::load_from_iter(["prog", "--bogus"]).unwrap_err();
    assert!(matches!(err, OrthoError::CliParsing(_)));
}

#[rstest]
fn invalid_cli_wrong_type_maps_error() {
    let err = OptionConfig::load_from_iter(["prog", "--maybe", "notanumber"]).unwrap_err();
    assert!(matches!(err, OrthoError::CliParsing(_)));
}

#[rstest]
fn invalid_cli_missing_required_maps_error() {
    figment::Jail::expect_with(|_| {
        let err = RequiredConfig::load_from_iter(["prog"]).unwrap_err();
        assert!(matches!(err, OrthoError::Merge { .. }));
        Ok(())
    });
}

#[rstest]
fn invalid_cli_duplicate_flag_maps_error() {
    let err =
        TestConfig::load_from_iter(["prog", "--sample-value", "foo", "--sample-value", "bar"])
            .unwrap_err();
    assert!(matches!(err, OrthoError::CliParsing(_)));
}

#[rstest]
fn merges_cli_into_figment() {
    use figment::{Figment, Profile, providers::Serialized};

    let cli = TestConfig {
        sample_value: Some("hi".into()),
        other: Some("there".into()),
    };

    let cfg: TestConfig = Figment::new()
        .merge(Serialized::from(cli, Profile::Default))
        .extract()
        .expect("extract");

    assert_eq!(cfg.sample_value.as_deref(), Some("hi"));
    assert_eq!(cfg.other.as_deref(), Some("there"));
}

#[rstest]
fn option_field_cli_present() {
    let cfg = OptionConfig::load_from_iter(["prog", "--maybe", "5"]).expect("load");
    assert_eq!(cfg.maybe, Some(5));
}

#[rstest]
fn option_field_cli_absent() {
    let cfg = OptionConfig::load_from_iter(["prog"]).expect("load");
    assert_eq!(cfg.maybe, None);
}

#[rstest]
fn resolves_short_flag_conflict() {
    let cfg = ConflictConfig::load_from_iter(["prog", "-s", "one", "-S", "two"]).expect("load");
    assert_eq!(cfg.second.as_deref(), Some("one"));
    assert_eq!(cfg.sample.as_deref(), Some("two"));
}

#[rstest]
fn config_path_env_var() {
    figment::Jail::expect_with(|j| {
        j.create_file("alt.toml", "sample_value = \"from_env\"\nother = \"val\"")?;
        j.set_env("CONFIG_PATH", "alt.toml");

        let cfg = TestConfig::load_from_iter(["prog"]).expect("load");
        assert_eq!(cfg.sample_value.as_deref(), Some("from_env"));
        assert_eq!(cfg.other.as_deref(), Some("val"));
        Ok(())
    });
}

#[rstest]
fn config_path_cli_overrides_default() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "sample_value = \"default\"\nother = \"d\"")?;
        j.create_file("alt.toml", "sample_value = \"alt\"\nother = \"a\"")?;

        let cfg = TestConfig::load_from_iter(["prog", "--config-path", "alt.toml"]).expect("load");
        assert_eq!(cfg.sample_value.as_deref(), Some("alt"));
        assert_eq!(cfg.other.as_deref(), Some("a"));
        Ok(())
    });
}

#[rstest]
fn missing_config_file_is_ignored() {
    figment::Jail::expect_with(|j| {
        j.set_env("CONFIG_PATH", "nope.toml");

        let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "cli", "--other", "val"])
            .expect("load");
        assert_eq!(cfg.sample_value.as_deref(), Some("cli"));
        assert_eq!(cfg.other.as_deref(), Some("val"));
        Ok(())
    });
}

// Windows lacks XDG support
#[cfg(any(unix, target_os = "redox"))]
#[test]
fn loads_from_xdg_config() {
    figment::Jail::expect_with(|j| {
        let dir = j.create_dir("xdg")?;
        let abs = ortho_config::file::canonicalise(&dir).expect("canonicalise dir");
        j.create_file(
            dir.join("config.toml"),
            "sample_value = \"xdg\"\nother = \"val\"",
        )?;
        j.set_env("XDG_CONFIG_HOME", abs.to_str().expect("dir to string"));

        let cfg = TestConfig::load_from_iter(["prog"]).expect("load");
        assert_eq!(cfg.sample_value.as_deref(), Some("xdg"));
        assert_eq!(cfg.other.as_deref(), Some("val"));
        Ok(())
    });
}

// Windows lacks XDG support
#[cfg(any(unix, target_os = "redox"))]
#[cfg(feature = "yaml")]
#[test]
fn loads_from_xdg_yaml_config() {
    figment::Jail::expect_with(|j| {
        let dir = j.create_dir("xdg_yaml")?;
        let abs = ortho_config::file::canonicalise(&dir).expect("canonicalise dir");
        j.create_file(dir.join("config.yaml"), "sample_value: xdg\nother: val")?;
        j.set_env("XDG_CONFIG_HOME", abs.to_str().expect("dir to string"));

        let cfg = TestConfig::load_from_iter(["prog"]).expect("load");
        assert_eq!(cfg.sample_value.as_deref(), Some("xdg"));
        assert_eq!(cfg.other.as_deref(), Some("val"));
        Ok(())
    });
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct RenamedPathConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    sample: Option<String>,
    #[serde(skip)]
    #[ortho_config(cli_long = "config")]
    config_path: Option<std::path::PathBuf>,
}

#[rstest]
fn config_path_custom_flag() {
    figment::Jail::expect_with(|j| {
        j.create_file("alt.toml", "sample = \"file\"")?;
        let cfg =
            RenamedPathConfig::load_from_iter(["prog", "--config", "alt.toml"]).expect("load");
        assert_eq!(cfg.sample.as_deref(), Some("file"));
        assert!(
            cfg.config_path.is_none(),
            "config_path should not be retained post-merge"
        );
        Ok(())
    });
}

#[rstest]
fn config_path_custom_env() {
    figment::Jail::expect_with(|j| {
        j.create_file("alt.toml", "sample = \"env\"")?;
        j.set_env("CONFIG_PATH", "alt.toml");
        let cfg = RenamedPathConfig::load_from_iter(["prog"]).expect("load");
        assert_eq!(cfg.sample.as_deref(), Some("env"));
        assert!(
            cfg.config_path.is_none(),
            "config_path should not be retained post-merge"
        );
        Ok(())
    });
}

#[rstest]
fn config_path_cli_overrides_env() {
    figment::Jail::expect_with(|j| {
        j.create_file("env.toml", "sample = \"env\"")?;
        j.create_file("cli.toml", "sample = \"cli\"")?;
        j.set_env("CONFIG_PATH", "env.toml");
        let cfg =
            RenamedPathConfig::load_from_iter(["prog", "--config", "cli.toml"]).expect("load");
        assert_eq!(cfg.sample.as_deref(), Some("cli"));
        assert!(
            cfg.config_path.is_none(),
            "config_path should not be retained post-merge",
        );
        Ok(())
    });
}
