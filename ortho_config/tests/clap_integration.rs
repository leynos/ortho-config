//! Tests covering CLI integration and error handling.

#![allow(non_snake_case)]

use clap::Parser;
use ortho_config::{OrthoConfig, OrthoError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig)]
struct TestConfig {
    #[arg(long = "sample-value")]
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_value: Option<String>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    other: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig)]
struct OptionConfig {
    #[arg(long)]
    maybe: Option<u32>,
}

#[test]
fn parses_kebab_case_flags() {
    let cli = TestConfig::parse_from(["prog", "--sample-value", "hello", "--other", "val"]);
    assert_eq!(cli.sample_value.as_deref(), Some("hello"));
    assert_eq!(cli.other.as_deref(), Some("val"));
}

#[test]
fn cli_only_source() {
    let cli = TestConfig::parse_from(["prog", "--sample-value", "only", "--other", "x"]);
    let cfg = cli.load_and_merge().expect("load");
    assert_eq!(cfg.sample_value.as_deref(), Some("only"));
    assert_eq!(cfg.other.as_deref(), Some("x"));
}

#[test]
fn cli_overrides_other_sources() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "sample_value = \"file\"\nother = \"f\"")?;
        j.set_env("SAMPLE_VALUE", "env");
        j.set_env("OTHER", "e");
        let cli = TestConfig::parse_from(["prog", "--sample-value", "cli", "--other", "cli2"]);
        let cfg = cli.load_and_merge().expect("load");
        assert_eq!(cfg.sample_value.as_deref(), Some("cli"));
        assert_eq!(cfg.other.as_deref(), Some("cli2"));
        Ok(())
    });
}

#[test]
fn cli_combines_with_file() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "other = \"file\"")?;
        let cli = TestConfig::parse_from(["prog", "--sample-value", "cli", "--other", "cli2"]);
        let cfg = cli.load_and_merge().expect("load");
        assert_eq!(cfg.sample_value.as_deref(), Some("cli"));
        // CLI argument should override file value
        assert_eq!(cfg.other.as_deref(), Some("cli2"));
        Ok(())
    });
}

#[test]
fn invalid_cli_input_maps_error() {
    let err = TestConfig::try_parse_from(["prog", "--bogus"])
        .map_err(OrthoError::CliParsing)
        .unwrap_err();
    matches!(err, OrthoError::CliParsing(_));
}

#[test]
fn merges_cli_into_figment() {
    use figment::{Figment, Profile, providers::Serialized};

    let cli = TestConfig::parse_from(["prog", "--sample-value", "hi", "--other", "there"]);

    let cfg: TestConfig = Figment::new()
        .merge(Serialized::from(cli, Profile::Default))
        .extract()
        .expect("extract");

    assert_eq!(cfg.sample_value.as_deref(), Some("hi"));
    assert_eq!(cfg.other.as_deref(), Some("there"));
}

#[test]
fn option_field_cli_present() {
    let cli = OptionConfig::parse_from(["prog", "--maybe", "5"]);
    let cfg = cli.load_and_merge().expect("load");
    assert_eq!(cfg.maybe, Some(5));
}

#[test]
fn option_field_cli_absent() {
    let cli = OptionConfig::parse_from(["prog"]);
    let cfg = cli.load_and_merge().expect("load");
    assert_eq!(cfg.maybe, None);
}

#[test]
fn config_path_env_var() {
    figment::Jail::expect_with(|j| {
        j.create_file("alt.toml", "sample_value = \"from_env\"\nother = \"val\"")?;
        j.set_env("CONFIG_PATH", "alt.toml");

        let cli = TestConfig::parse_from(["prog"]);
        let cfg = cli.load_and_merge().expect("load");
        assert_eq!(cfg.sample_value.as_deref(), Some("from_env"));
        assert_eq!(cfg.other.as_deref(), Some("val"));
        Ok(())
    });
}

#[test]
fn missing_config_file_is_ignored() {
    figment::Jail::expect_with(|j| {
        j.set_env("CONFIG_PATH", "nope.toml");

        let cli = TestConfig::parse_from(["prog", "--sample-value", "cli", "--other", "val"]);
        let cfg = cli.load_and_merge().expect("load");
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
        let abs = std::fs::canonicalize(&dir).unwrap();
        j.create_file(
            dir.join("config.toml"),
            "sample_value = \"xdg\"\nother = \"val\"",
        )?;
        j.set_env("XDG_CONFIG_HOME", abs.to_str().unwrap());

        let cli = TestConfig::parse_from(["prog"]);
        let cfg = cli.load_and_merge().expect("load");
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
        let abs = std::fs::canonicalize(&dir).unwrap();
        j.create_file(dir.join("config.yaml"), "sample_value: xdg\nother: val")?;
        j.set_env("XDG_CONFIG_HOME", abs.to_str().unwrap());

        let cli = TestConfig::parse_from(["prog"]);
        let cfg = cli.load_and_merge().expect("load");
        assert_eq!(cfg.sample_value.as_deref(), Some("xdg"));
        assert_eq!(cfg.other.as_deref(), Some("val"));
        Ok(())
    });
}
