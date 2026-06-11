//! BDD coverage for localised help rendering in the `hello_world` example.

use anyhow::{Result, ensure};
use assert_cmd::Command as AssertCommand;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenarios, then, when};
use test_helpers::text::{normalize_scalar, strip_isolates};

#[derive(Debug, Default, ScenarioState)]
struct HelpState {
    locale: Slot<String>,
    output: Slot<String>,
}

#[fixture]
fn help_state() -> HelpState {
    HelpState::default()
}

scenarios!(
    "tests/features/localised_help.feature",
    fixtures = [help_state: HelpState]
);

#[given("the user's locale is {locale}")]
fn user_locale(help_state: &HelpState, locale: String) {
    let normalized_locale = normalize_scalar(&locale);
    let lang = match normalized_locale.as_str() {
        "ja" => "ja_JP.UTF-8",
        "en-US" => "en_US.UTF-8",
        other => other,
    };
    help_state.locale.set(lang.to_owned());
}

#[when("the user renders the hello-world long help")]
fn render_long_help(help_state: &HelpState) -> Result<()> {
    let lang = help_state
        .locale
        .with_ref(Clone::clone)
        .unwrap_or_else(|| String::from("en_US.UTF-8"));

    #[expect(
        deprecated,
        reason = "cargo_bin is the standard assert_cmd API in this test suite"
    )]
    let mut command = AssertCommand::cargo_bin("hello_world")?;
    command
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("LANG")
        .env("LANG", lang)
        .env("RUST_BACKTRACE", "0")
        .arg("--help");

    let output = command.output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stdout.is_empty() {
        stderr.into_owned()
    } else {
        stdout.into_owned()
    };
    help_state.output.set(combined.replace("\r\n", "\n"));
    Ok(())
}

#[then("the about line contains the Japanese greeting copy")]
fn about_contains_japanese_copy(help_state: &HelpState) -> Result<()> {
    assert_output_contains(help_state, "設定、環境変数、CLIにまたがる階層化された挨拶")
}

#[then("the greet subcommand help contains the Japanese greeting copy")]
fn greet_help_contains_japanese_copy(help_state: &HelpState) -> Result<()> {
    assert_output_contains(
        help_state,
        "設定されたテンプレートを使用してフレンドリーな挨拶を表示します。",
    )
}

#[then("the about line contains the English localised copy")]
fn about_contains_english_copy(help_state: &HelpState) -> Result<()> {
    assert_output_contains(help_state, "Use hello-world to explore layered greetings")
}

#[then("the greet subcommand help contains the English localised copy")]
fn greet_help_contains_english_copy(help_state: &HelpState) -> Result<()> {
    assert_output_contains(
        help_state,
        "Prints a friendly greeting using any configured templates.",
    )
}

fn assert_output_contains(help_state: &HelpState, expected: &str) -> Result<()> {
    let output = help_state.output.with_ref(Clone::clone).unwrap_or_default();
    let normalized_output = strip_isolates(&output);
    ensure!(
        normalized_output.contains(expected),
        "expected help output to contain {expected:?}; output was: {output:?}"
    );
    Ok(())
}
