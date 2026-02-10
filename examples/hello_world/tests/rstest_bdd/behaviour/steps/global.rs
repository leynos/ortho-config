//! Step definitions for the `hello_world` example.
//! Drive the binary and assert its observable outputs.
use crate::behaviour::config::SampleConfigError;
use crate::behaviour::harness::Harness;
use anyhow::{Context, Result, anyhow};
use camino::Utf8PathBuf;
use hello_world::cli::GlobalArgs;
use ortho_config::MergeComposer;
use ortho_config::serde_json::{self, Value};
use rstest_bdd_macros::{given, then, when};
use serde::Deserialize;
use test_helpers::text::normalize_scalar as normalize_test_scalar;

fn validate_env_key(key: &str) -> Result<()> {
    anyhow::ensure!(
        !key.trim().is_empty(),
        "environment variable key must not be empty"
    );
    Ok(())
}

fn normalize_step_scalar(value: &str) -> String {
    normalize_test_scalar(value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LayerProvenance {
    Defaults,
    Environment,
    Cli,
    File,
}

#[derive(Debug, Deserialize)]
struct LayerInput {
    provenance: LayerProvenance,
    value: Value,
    path: Option<String>,
}

/// Runs the binary without additional arguments.
#[when("I run the hello world example")]
pub fn run_without_args(#[from(hello_world_harness)] harness: &mut Harness) -> Result<()> {
    harness.run_hello(None)
}

#[when("I run the hello world example with arguments {arguments}")]
pub fn run_with_args(
    #[from(hello_world_harness)] harness: &mut Harness,
    arguments: String,
) -> Result<()> {
    harness.run_hello(Some(normalize_step_scalar(&arguments)))
}

#[then("the command succeeds")]
pub fn command_succeeds(#[from(hello_world_harness)] harness: &mut Harness) -> Result<()> {
    harness.assert_success()
}

#[then("the command fails")]
pub fn command_fails(#[from(hello_world_harness)] harness: &mut Harness) -> Result<()> {
    harness.assert_failure()
}

#[then("stdout contains {expected_stdout}")]
pub fn stdout_contains(
    #[from(hello_world_harness)] harness: &mut Harness,
    expected_stdout: String,
) -> Result<()> {
    let expected_stdout = normalize_step_scalar(&expected_stdout);
    harness.assert_stdout_contains(&expected_stdout)
}

/// Ensures the reported version matches the crate metadata.
#[then("stdout contains the hello world version")]
pub fn stdout_contains_version(#[from(hello_world_harness)] harness: &mut Harness) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let expected = format!("hello-world {version}");
    harness.assert_stdout_contains(&expected)
}

#[then("stderr contains {expected_stderr}")]
pub fn stderr_contains(
    #[from(hello_world_harness)] harness: &mut Harness,
    expected_stderr: String,
) -> Result<()> {
    let expected_stderr = normalize_step_scalar(&expected_stderr);
    harness.assert_stderr_contains(&expected_stderr)
}

#[given("the environment contains {env_key} = {env_value}")]
pub fn environment_contains(
    #[from(hello_world_harness)] harness: &mut Harness,
    env_key: String,
    env_value: String,
) -> Result<()> {
    let env_key = normalize_step_scalar(&env_key);
    let env_value = normalize_step_scalar(&env_value);
    validate_env_key(&env_key)?;
    harness.set_env(&env_key, &env_value);
    Ok(())
}

#[given("the environment does not contain {env_key}")]
pub fn environment_does_not_contain(
    #[from(hello_world_harness)] harness: &mut Harness,
    env_key: String,
) -> Result<()> {
    let env_key = normalize_step_scalar(&env_key);
    validate_env_key(&env_key)?;
    harness.remove_env(&env_key);
    Ok(())
}

/// Writes docstring contents to the default configuration file.
#[given("the hello world config file contains:")]
pub fn config_file(
    #[from(hello_world_harness)] harness: &Harness,
    docstring: String,
) -> Result<()> {
    harness.write_config(&docstring)
}

/// Writes docstring contents to a named file.
#[given("the file {name} contains:")]
pub fn named_file_contains(
    #[from(hello_world_harness)] harness: &Harness,
    name: String,
    docstring: String,
) -> Result<()> {
    let name = normalize_step_scalar(&name);
    harness.write_named_file(&name, &docstring)
}

/// Writes docstring contents to the XDG config home directory.
#[given("the XDG config home contains:")]
pub fn xdg_config_home_contains(
    #[from(hello_world_harness)] harness: &mut Harness,
    docstring: String,
) -> Result<()> {
    harness.write_xdg_config_home(&docstring)
}

/// Initializes the scenario using a repository sample configuration.
#[given("I start from the sample hello world config {sample}")]
pub fn start_from_sample_config(
    #[from(hello_world_harness)] harness: &Harness,
    sample: String,
) -> Result<()> {
    let sample = normalize_step_scalar(&sample);
    harness.write_sample_config(&sample)
}

#[given("I start from a missing or invalid sample config {sample_name}")]
pub fn start_from_invalid_sample_config(
    #[from(hello_world_harness)] harness: &Harness,
    sample_name: String,
) -> Result<()> {
    let sample_name = normalize_step_scalar(&sample_name);
    match harness.try_write_sample_config(&sample_name) {
        Ok(()) => {
            return Err(anyhow!(
                "expected sample config {sample_name:?} to be missing or invalid"
            ));
        }
        Err(
            SampleConfigError::OpenSample { .. }
            | SampleConfigError::ReadSample { .. }
            | SampleConfigError::WriteSample { .. }
            | SampleConfigError::InvalidName { .. }
            | SampleConfigError::OpenConfigDir { .. },
        ) => {}
    }
    Ok(())
}

fn compose_declarative_globals_from_contents(harness: &mut Harness, contents: &str) -> Result<()> {
    let inputs: Vec<LayerInput> =
        serde_json::from_str(contents).context("valid JSON describing declarative layers")?;
    let mut composer = MergeComposer::new();
    for input in inputs {
        match input.provenance {
            LayerProvenance::Defaults => composer.push_defaults(input.value),
            LayerProvenance::Environment => composer.push_environment(input.value),
            LayerProvenance::Cli => composer.push_cli(input.value),
            LayerProvenance::File => {
                let path = input.path.map(Utf8PathBuf::from);
                composer.push_file(input.value, path);
            }
        }
    }
    let globals = GlobalArgs::merge_from_layers(composer.layers())
        .context("declarative merge should succeed for globals")?;
    harness.set_declarative_globals(globals);
    Ok(())
}

#[given("I compose hello world globals from declarative layers:")]
pub fn compose_declarative_globals(
    #[from(hello_world_harness)] harness: &mut Harness,
    docstring: String,
) -> Result<()> {
    compose_declarative_globals_from_contents(harness, &docstring)
}

#[then("the declarative globals recipient is {expected}")]
pub fn assert_declarative_recipient(
    #[from(hello_world_harness)] harness: &Harness,
    expected: String,
) -> Result<()> {
    let expected = normalize_step_scalar(&expected);
    harness.assert_declarative_recipient(&expected)
}

#[then("the declarative globals salutations are:")]
pub fn assert_declarative_salutations(
    #[from(hello_world_harness)] harness: &Harness,
    docstring: String,
) -> Result<()> {
    let expected: Vec<String> = docstring
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    harness.assert_declarative_salutations(&expected)
}

#[cfg(test)]
pub(crate) fn compose_declarative_globals_from_contents_for_tests(
    harness: &mut Harness,
    contents: &str,
) -> Result<()> {
    compose_declarative_globals_from_contents(harness, contents)
}
