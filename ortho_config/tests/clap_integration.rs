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
