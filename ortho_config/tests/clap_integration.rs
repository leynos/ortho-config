#![allow(non_snake_case)]

use ortho_config::{OrthoConfig, OrthoError};
use serde::Deserialize;

#[derive(Debug, Deserialize, OrthoConfig)]
struct TestConfig {
    sample_value: String,
    other: String,
}

#[allow(non_camel_case_types, non_snake_case)]
type TestConfigCli = __TestConfigCliMod::__TestConfigCli;

#[derive(Debug, Deserialize, OrthoConfig)]
struct OptionConfig {
    maybe: Option<u32>,
}

#[test]
fn parses_kebab_case_flags() {
    use clap::Parser;
    let cli = TestConfigCli::parse_from(["prog", "--sample-value", "hello", "--other", "val"]);
    assert_eq!(cli.sample_value.as_deref(), Some("hello"));
    assert_eq!(cli.other.as_deref(), Some("val"));
}

#[test]
fn cli_only_source() {
    let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "only", "--other", "x"])
        .expect("load");
    assert_eq!(cfg.sample_value, "only");
    assert_eq!(cfg.other, "x");
}

#[test]
fn cli_overrides_other_sources() {
    figment::Jail::expect_with(|j| {
        j.create_file("config.toml", "sample_value = \"file\"\nother = \"f\"")?;
        j.set_env("SAMPLE_VALUE", "env");
        j.set_env("OTHER", "e");
        let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "cli", "--other", "cli2"])
            .expect("load");
        assert_eq!(cfg.sample_value, "cli");
        assert_eq!(cfg.other, "cli2");
        Ok(())
    });
}

#[test]
fn cli_combines_with_file() {
    figment::Jail::expect_with(|j| {
        j.create_file("config.toml", "other = \"file\"")?;
        let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "cli", "--other", "cli2"])
            .expect("load");
        assert_eq!(cfg.sample_value, "cli");
        // CLI argument should override file value
        assert_eq!(cfg.other, "cli2");
        Ok(())
    });
}

#[test]
fn invalid_cli_input_maps_error() {
    let err = TestConfig::load_from_iter(["prog", "--bogus"]).unwrap_err();
    matches!(err, OrthoError::CliParsing(_));
}

#[test]
fn merges_cli_into_figment() {
    use clap::Parser;
    use figment::{Figment, Profile, providers::Serialized};

    let cli = TestConfigCli::parse_from(["prog", "--sample-value", "hi", "--other", "there"]);

    let cfg: TestConfig = Figment::new()
        .merge(Serialized::from(cli, Profile::Default))
        .extract()
        .expect("extract");

    assert_eq!(cfg.sample_value, "hi");
    assert_eq!(cfg.other, "there");
}

#[test]
fn option_field_cli_present() {
    let cfg = OptionConfig::load_from_iter(["prog", "--maybe", "5"]).expect("load");
    assert_eq!(cfg.maybe, Some(5));
}

#[test]
fn option_field_cli_absent() {
    let cfg = OptionConfig::load_from_iter(["prog"]).expect("load");
    assert_eq!(cfg.maybe, None);
}

#[test]
fn config_path_env_var() {
    figment::Jail::expect_with(|j| {
        j.create_file("alt.toml", "sample_value = \"from_env\"\nother = \"val\"")?;
        j.set_env("CONFIG_PATH", "alt.toml");

        let cfg = TestConfig::load_from_iter(["prog"]).expect("load");
        assert_eq!(cfg.sample_value, "from_env");
        assert_eq!(cfg.other, "val");
        Ok(())
    });
}

#[test]
fn missing_config_file_is_ignored() {
    figment::Jail::expect_with(|j| {
        j.set_env("CONFIG_PATH", "nope.toml");

        let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "cli", "--other", "val"])
            .expect("load");
        assert_eq!(cfg.sample_value, "cli");
        assert_eq!(cfg.other, "val");
        Ok(())
    });
}
