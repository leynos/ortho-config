//! Tests focused on direct CLI parsing and merging behaviour.

use super::common::{
    ExpectedConfig, TestConfig, ToAnyhow, assert_config_eq, assert_config_values, run_config_case,
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
        // Collections merge by appending, so CLI values follow the default
        // "Hello" rather than replacing it.
        salutations: &["Hello", "Hello", "All"],
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
    // Run inside the jail so concurrent tests' environment mutations cannot
    // leak into an unjailed load.
    run_config_case::<TestConfig, _>(&[], &[], args, |cfg| {
        assert_config_eq(cfg, &expected).to_anyhow()
    })?;
    Ok(())
}

struct MergeCase {
    files: &'static [(&'static str, &'static str)],
    env: &'static [(&'static str, &'static str)],
    cli_args: &'static [&'static str],
    expected_sample: Option<&'static str>,
    expected_other: Option<&'static str>,
}

#[rstest]
#[case::overrides(MergeCase {
    files: &[ (".config.toml", "sample_value = \"file\"\nother = \"f\"") ],
    env: &[ ("SAMPLE_VALUE", "env"), ("OTHER", "e") ],
    cli_args: &["prog", "--sample-value", "cli", "--other", "cli2"],
    expected_sample: Some("cli"),
    expected_other: Some("cli2"),
})]
#[case::combines(MergeCase {
    files: &[ (".config.toml", "other = \"file\"") ],
    env: &[],
    cli_args: &["prog", "--sample-value", "cli", "--other", "cli2"],
    expected_sample: Some("cli"),
    expected_other: Some("cli2"),
})]
fn cli_merges_with_other_sources(#[case] case: MergeCase) -> Result<()> {
    run_config_case::<TestConfig, _>(case.files, case.env, case.cli_args, |cfg| {
        assert_config_values(cfg, case.expected_sample, case.expected_other)
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
        .map_err(anyhow::Error::from)?;

    assert_config_values(&cfg, Some("hi"), Some("there"))
}
