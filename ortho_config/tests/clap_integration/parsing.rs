//! Tests focused on direct CLI parsing and merging behaviour.

use super::common::{
    ExpectedConfig, TestConfig, ToAnyhow, assert_config_eq, assert_config_values,
    assert_is_excited, run_config_case,
};
use anyhow::Result;
use rstest::rstest;

/// Table-driven check that each CLI argument combination yields the expected
/// `TestConfig` values, with an empty injected environment so concurrent tests
/// cannot interfere.
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
    // Inject an empty environment so concurrent tests cannot affect this load.
    run_config_case::<TestConfig, _>(&[], &[], args, |cfg| {
        assert_config_eq(cfg, &expected).to_anyhow()
    })?;
    Ok(())
}

/// One row of the boolean-precedence matrix.
///
/// `files` and `env` supply the lower-precedence rungs; `cli_args` supplies the
/// command line. The case name records which rung is expected to win.
struct BoolCase {
    files: &'static [(&'static str, &'static str)],
    env: &'static [(&'static str, &'static str)],
    cli_args: &'static [&'static str],
    expected_is_excited: bool,
}

#[rstest]
// Absent flag: the struct default stands.
#[case::absent(BoolCase {
    files: &[],
    env: &[],
    cli_args: &["prog"],
    expected_is_excited: false,
})]
// Bare flag still means true.
#[case::bare_flag_sets_true(BoolCase {
    files: &[],
    env: &[],
    cli_args: &["prog", "--is-excited"],
    expected_is_excited: true,
})]
// An explicit value is accepted on the long flag.
#[case::explicit_true(BoolCase {
    files: &[],
    env: &[],
    cli_args: &["prog", "--is-excited=true"],
    expected_is_excited: true,
})]
// An explicit false matches the default and must not error.
#[case::explicit_false(BoolCase {
    files: &[],
    env: &[],
    cli_args: &["prog", "--is-excited=false"],
    expected_is_excited: false,
})]
// The short flag accepts the same optional value. The derive assigns `i`
// to `is_excited` because `r` and `s` are already claimed.
#[case::short_explicit_false(BoolCase {
    files: &[],
    env: &[],
    cli_args: &["prog", "-i=false"],
    expected_is_excited: false,
})]
// No CLI flag: a file value must survive untouched.
#[case::file_true_without_flag(BoolCase {
    files: &[(".config.toml", "is_excited = true")],
    env: &[],
    cli_args: &["prog"],
    expected_is_excited: true,
})]
// No CLI flag: an environment value must survive untouched.
#[case::env_true_without_flag(BoolCase {
    files: &[],
    env: &[("IS_EXCITED", "true")],
    cli_args: &["prog"],
    expected_is_excited: true,
})]
// The file value must also survive when an *unrelated* flag pushes the CLI
// layer. The rows above pass with no CLI layer at all, so they cannot see the
// `SetTrue` default: with any other argument typed, that absent-flag `false`
// reaches the layer and outranks the file.
#[case::file_true_with_unrelated_flag(BoolCase {
    files: &[(".config.toml", "is_excited = true")],
    env: &[],
    cli_args: &["prog", "--sample-value", "hello"],
    expected_is_excited: true,
})]
// As above, for an environment value: the CLI layer outranks the environment
// layer, so the same spurious `false` would clobber it.
#[case::env_true_with_unrelated_flag(BoolCase {
    files: &[],
    env: &[("IS_EXCITED", "true")],
    cli_args: &["prog", "--sample-value", "hello"],
    expected_is_excited: true,
})]
// The CLI clears a file that set true.
#[case::cli_false_clears_file_true(BoolCase {
    files: &[(".config.toml", "is_excited = true")],
    env: &[],
    cli_args: &["prog", "--is-excited=false"],
    expected_is_excited: false,
})]
// The CLI clears an environment value that set true.
#[case::cli_false_clears_env_true(BoolCase {
    files: &[],
    env: &[("IS_EXCITED", "true")],
    cli_args: &["prog", "--is-excited=false"],
    expected_is_excited: false,
})]
// The CLI clears an environment value overriding a file that set true.
#[case::cli_false_clears_env_over_file_true(BoolCase {
    files: &[(".config.toml", "is_excited = true")],
    env: &[("IS_EXCITED", "true")],
    cli_args: &["prog", "--is-excited=false"],
    expected_is_excited: false,
})]
fn boolean_flags_control_precedence(#[case] case: BoolCase) -> Result<()> {
    // Only the boolean is asserted: the rows that push a CLI layer do so with
    // an unrelated argument, which legitimately changes another field.
    run_config_case::<TestConfig, _>(case.files, case.env, case.cli_args, |cfg| {
        assert_is_excited(cfg, case.expected_is_excited).to_anyhow()
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
