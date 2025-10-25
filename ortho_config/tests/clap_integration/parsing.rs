//! Tests focused on direct CLI parsing and merging behaviour.

use super::common::{
    ExpectedConfig, OrthoResultExt, TestConfig, assert_config_eq, assert_config_values,
    load_from_iter, run_config_case,
};
use anyhow::Result;
use rstest::rstest;

#[rstest]
#[case::defaults(&["prog"], ExpectedConfig::default())]
#[case::sample_and_other(
    &["prog", "--sample-value", "hello", "--other", "val"],
    ExpectedConfig { sample_value: Some("hello"), other: Some("val"), ..ExpectedConfig::default() }
)]
#[case::recipient_and_salutations(
    &["prog", "--recipient", "Team", "--salutations", "Hello", "--salutations", "All", "--is-excited"],
    ExpectedConfig {
        recipient: "Team",
        salutations: &["Hello", "All"],
        is_excited: true,
        ..ExpectedConfig::default()
    }
)]
#[case::quiet_flag(
    &["prog", "--is-quiet"],
    ExpectedConfig {
        is_quiet: true,
        ..ExpectedConfig::default()
    }
)]
fn parses_cli_arguments(
    #[case] args: &[&'static str],
    #[case] expected: ExpectedConfig,
) -> Result<()> {
    let cfg = load_from_iter(args.iter().copied()).to_anyhow()?;
    assert_config_eq(&cfg, &expected).to_anyhow()
}

#[rstest]
#[case::overrides(
    &[ (".config.toml", "sample_value = \"file\"\nother = \"f\"") ],
    &[ ("SAMPLE_VALUE", "env"), ("OTHER", "e") ],
    &["prog", "--sample-value", "cli", "--other", "cli2"],
    Some("cli"),
    Some("cli2")
)]
#[case::combines(
    &[ (".config.toml", "other = \"file\"") ],
    &[],
    &["prog", "--sample-value", "cli", "--other", "cli2"],
    Some("cli"),
    Some("cli2")
)]
fn cli_merges_with_other_sources(
    #[case] files: &[(&str, &str)],
    #[case] env: &[(&str, &str)],
    #[case] cli_args: &[&'static str],
    #[case] expected_sample: Option<&'static str>,
    #[case] expected_other: Option<&'static str>,
) -> Result<()> {
    run_config_case::<TestConfig, _>(files, env, cli_args, |cfg| {
        assert_config_values(&cfg, expected_sample, expected_other)
    })?;
    Ok(())
}

#[rstest]
fn merges_cli_into_figment() -> Result<()> {
    use figment::{Figment, Profile, providers::Serialized};

    let cli = TestConfig {
        sample_value: Some("hi".into()),
        other: Some("there".into()),
        ..TestConfig::default()
    };

    let cfg: TestConfig = Figment::new()
        .merge(Serialized::from(cli, Profile::Default))
        .extract()
        .to_anyhow()?;

    assert_config_values(&cfg, Some("hi"), Some("there"))
}
