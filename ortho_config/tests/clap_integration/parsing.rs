//! Tests focused on direct CLI parsing and merging behaviour.

use super::common::{assert_config_values, with_jail, OrthoResultExt, TestConfig};
use anyhow::Result;
use rstest::rstest;

#[rstest]
#[case::kebab(&["prog", "--sample-value", "hello", "--other", "val"], Some("hello"), Some("val"))]
#[case::short(&["prog", "-s", "hi", "-o", "val"], Some("hi"), Some("val"))]
#[case::only(&["prog", "--sample-value", "only", "--other", "x"], Some("only"), Some("x"))]
fn loads_cli_arguments(
    #[case] args: &[&str],
    #[case] expected_sample: Option<&str>,
    #[case] expected_other: Option<&str>,
) -> Result<()> {
    let cfg = TestConfig::load_from_iter(args.iter().copied()).to_anyhow()?;
    assert_config_values(&cfg, expected_sample, expected_other)
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
    #[case] cli_args: &[&str],
    #[case] expected_sample: Option<&str>,
    #[case] expected_other: Option<&str>,
) -> Result<()> {
    with_jail(|j| {
        for (path, contents) in files {
            j.create_file(path, contents)?;
        }
        for (key, value) in env {
            j.set_env(key, value);
        }
        let cfg = TestConfig::load_from_iter(cli_args.iter().copied()).to_anyhow()?;
        assert_config_values(&cfg, expected_sample, expected_other)
    })
}

#[rstest]
fn merges_cli_into_figment() -> Result<()> {
    use figment::{providers::Serialized, Figment, Profile};

    let cli = TestConfig {
        sample_value: Some("hi".into()),
        other: Some("there".into()),
    };

    let cfg: TestConfig = Figment::new()
        .merge(Serialized::from(cli, Profile::Default))
        .extract()
        .to_anyhow()?;

    assert_config_values(&cfg, Some("hi"), Some("there"))
}
