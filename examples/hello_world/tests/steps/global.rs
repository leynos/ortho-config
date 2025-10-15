#![allow(
    unfulfilled_lint_expectations,
    reason = "Clippy 1.81 does not emit needless_pass_by_value for cucumber steps yet; expectations document the signature requirements."
)]
//! Step definitions for the `hello_world` example.
//! Drive the binary and assert its outputs.
use crate::{SampleConfigError, World};
use camino::Utf8PathBuf;
use cucumber::gherkin::Step as GherkinStep;
use cucumber::{given, then, when};
use hello_world::cli::GlobalArgs;
use ortho_config::MergeComposer;
use ortho_config::serde_json::{self, Value};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub(crate) struct CapturedString(String);

impl CapturedString {
    fn as_str(&self) -> &str {
        self.0.as_str()
    }

    fn into_string(self) -> String {
        self.0
    }
}

impl std::str::FromStr for CapturedString {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.to_owned()))
    }
}

impl From<CapturedString> for String {
    fn from(value: CapturedString) -> Self {
        value.into_string()
    }
}

impl AsRef<str> for CapturedString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

fn extract_docstring(step: &GherkinStep) -> &str {
    step.docstring()
        .expect("config docstring provided for hello world example")
}

#[derive(Debug, Deserialize)]
struct LayerInput {
    provenance: String,
    value: Value,
    path: Option<String>,
}

/// Runs the binary without additional arguments.
#[when("I run the hello world example")]
pub async fn run_without_args(world: &mut World) {
    world.run_hello(None).await;
}

#[when(expr = "I run the hello world example with arguments {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
// Step captures arrive as owned capture values from cucumber; lend them
// to the world helper for tokenisation.
pub async fn run_with_args(world: &mut World, args: CapturedString) {
    world.run_hello(Some(args.as_str())).await;
}

#[then("the command succeeds")]
pub fn command_succeeds(world: &mut World) {
    world.assert_success();
}

#[then("the command fails")]
pub fn command_fails(world: &mut World) {
    world.assert_failure();
}

#[then(expr = "stdout contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
// Step captures arrive as owned capture values from cucumber; borrow them
// for assertions so the captured text remains available.
pub fn stdout_contains(world: &mut World, expected: CapturedString) {
    world.assert_stdout_contains(expected.as_str());
}

#[then(expr = "stderr contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
// Step captures arrive as owned capture values from cucumber; borrow them
// for assertions so the captured text remains available.
pub fn stderr_contains(world: &mut World, expected: CapturedString) {
    world.assert_stderr_contains(expected.as_str());
}

#[given(expr = "the environment contains {string} = {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
pub fn environment_contains(world: &mut World, key: CapturedString, value: CapturedString) {
    world.set_env(key, value);
}

#[given(expr = "the environment does not contain {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
pub fn environment_does_not_contain(world: &mut World, key: CapturedString) {
    world.remove_env(key.as_str());
}

/// Writes docstring contents to the default configuration file.
#[given("the hello world config file contains:")]
pub fn config_file(world: &mut World, step: &GherkinStep) {
    let contents = extract_docstring(step);
    world.write_config(contents);
}

/// Writes docstring contents to a named file.
#[given(expr = "the file {string} contains:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
pub fn named_file_contains(world: &mut World, name: CapturedString, step: &GherkinStep) {
    let contents = extract_docstring(step);
    world.write_named_file(name.as_str(), contents);
}

/// Writes docstring contents to the XDG config home directory.
#[given("the XDG config home contains:")]
pub fn xdg_config_home_contains(world: &mut World, step: &GherkinStep) {
    let contents = extract_docstring(step);
    world.write_xdg_config_home(contents);
}

/// Initialises the scenario using a repository sample configuration.
#[given(expr = "I start from the sample hello world config {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
pub fn start_from_sample_config(world: &mut World, sample: CapturedString) {
    world.write_sample_config(sample.as_str());
}

#[given(expr = "I start from a missing or invalid sample config {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
pub fn start_from_invalid_sample_config(world: &mut World, sample: CapturedString) {
    let sample_name = sample.into_string();
    match world.try_write_sample_config(sample_name.as_str()) {
        Ok(()) => panic!("expected sample config {sample_name:?} to be missing or invalid"),
        Err(
            SampleConfigError::OpenSample { .. }
            | SampleConfigError::ReadSample { .. }
            | SampleConfigError::WriteSample { .. },
        ) => {}
        Err(err) => panic!("unexpected sample config error: {err}"),
    }
}

#[given("I compose hello world globals from declarative layers:")]
pub fn compose_declarative_globals(world: &mut World, step: &GherkinStep) {
    let contents = extract_docstring(step);
    let inputs: Vec<LayerInput> =
        serde_json::from_str(contents).expect("valid JSON describing declarative layers");
    let mut composer = MergeComposer::new();
    for input in inputs {
        match input.provenance.as_str() {
            "defaults" => composer.push_defaults(input.value),
            "environment" => composer.push_environment(input.value),
            "cli" => composer.push_cli(input.value),
            "file" => {
                let path = input.path.map(Utf8PathBuf::from);
                composer.push_file(input.value, path);
            }
            other => panic!("unknown provenance {other}"),
        }
    }
    let globals = GlobalArgs::merge_from_layers(composer.layers())
        .expect("declarative merge should succeed for globals");
    world.set_declarative_globals(globals);
}

#[then(expr = "the declarative globals recipient is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
pub fn assert_declarative_recipient(world: &mut World, expected: CapturedString) {
    world.assert_declarative_recipient(expected.as_str());
}

#[then("the declarative globals salutations are:")]
pub fn assert_declarative_salutations(world: &mut World, step: &GherkinStep) {
    let contents = extract_docstring(step);
    let expected: Vec<String> = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    world.assert_declarative_salutations(&expected);
}
