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
// Step captures arrive as owned `String` values from cucumber; forward them
// to the world helper for tokenisation.
pub async fn run_with_args(world: &mut World, args: String) {
    world.run_hello(Some(args)).await;
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
// Step captures arrive as owned `String` values from cucumber; forward them
// to the world helper so assertions can consume the owned capture directly.
pub fn stdout_contains(world: &mut World, expected: String) {
    world.assert_stdout_contains(expected);
}

#[then(expr = "stderr contains {string}")]
// Step captures arrive as owned `String` values from cucumber; forward them
// to the world helper so assertions can consume the owned capture directly.
pub fn stderr_contains(world: &mut World, expected: String) {
    world.assert_stderr_contains(expected);
}

#[given(expr = "the environment contains {string} = {string}")]
pub fn environment_contains(world: &mut World, key: String, value: String) {
    world.set_env(key, value);
}

#[given(expr = "the environment does not contain {string}")]
pub fn environment_does_not_contain(world: &mut World, key: String) {
    world.remove_env(key);
}

/// Writes docstring contents to the default configuration file.
#[given("the hello world config file contains:")]
pub fn config_file(world: &mut World, step: &GherkinStep) {
    let contents = extract_docstring(step);
    world.write_config(contents);
}

/// Writes docstring contents to a named file.
#[given(expr = "the file {string} contains:")]
pub fn named_file_contains(world: &mut World, name: String, step: &GherkinStep) {
    let contents = extract_docstring(step);
    world.write_named_file(name, contents);
}

/// Writes docstring contents to the XDG config home directory.
#[given("the XDG config home contains:")]
pub fn xdg_config_home_contains(world: &mut World, step: &GherkinStep) {
    let contents = extract_docstring(step);
    world.write_xdg_config_home(contents);
}

/// Initialises the scenario using a repository sample configuration.
#[given(expr = "I start from the sample hello world config {string}")]
pub fn start_from_sample_config(world: &mut World, sample: String) {
    world.write_sample_config(sample);
}

#[given(expr = "I start from a missing or invalid sample config {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Step captures arrive owned from cucumber; cloning is required for diagnostic panics"
)]
pub fn start_from_invalid_sample_config(world: &mut World, sample_name: String) {
    match world.try_write_sample_config(sample_name.clone()) {
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
pub fn assert_declarative_recipient(world: &mut World, expected: String) {
    world.assert_declarative_recipient(expected);
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
