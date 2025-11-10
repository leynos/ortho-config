//! Step definitions for the `hello_world` example.
//! Drive the binary and assert its outputs.
#![expect(
    clippy::shadow_reuse,
    reason = "Cucumber step macros rebind step arguments during code generation"
)]

use crate::{SampleConfigError, World};
use anyhow::{Context, Result, anyhow};
use camino::Utf8PathBuf;
use cucumber::gherkin::Step as GherkinStep;
use cucumber::{given, then, when};
use hello_world::cli::GlobalArgs;
use ortho_config::MergeComposer;
use ortho_config::serde_json::{self, Value};
use serde::Deserialize;

fn extract_docstring(step: &GherkinStep) -> Result<&str> {
    step.docstring()
        .map(String::as_str)
        .ok_or_else(|| anyhow!("config docstring provided for hello world example"))
}

async fn run_with_args_inner(world: &mut World, args: String) -> Result<()> {
    world.run_hello(Some(args)).await?;
    Ok(())
}

fn validate_env_key(key: &str) -> Result<()> {
    anyhow::ensure!(
        !key.trim().is_empty(),
        "environment variable key must not be empty"
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
struct LayerInput {
    provenance: String,
    value: Value,
    path: Option<String>,
}

/// Runs the binary without additional arguments.
#[when("I run the hello world example")]
pub async fn run_without_args(world: &mut World) -> Result<()> {
    world.run_hello(None).await?;
    Ok(())
}

#[when(expr = "I run the hello world example with arguments {string}")]
// Step captures arrive as owned `String` values from cucumber; forward them to
// the world helper for tokenisation while retaining ownership requirements of
// the async state machine.
pub async fn run_with_args(world: &mut World, arguments: String) -> Result<()> {
    run_with_args_inner(world, arguments).await
}

#[then("the command succeeds")]
pub fn command_succeeds(world: &mut World) -> Result<()> {
    world.assert_success()
}

#[then("the command fails")]
pub fn command_fails(world: &mut World) -> Result<()> {
    world.assert_failure()
}

#[then(expr = "stdout contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
// Step captures arrive as owned `String` values from cucumber; forward them
// to the world helper so assertions can consume the owned capture directly.
pub fn stdout_contains(world: &mut World, expected_stdout: String) -> Result<()> {
    world.assert_stdout_contains(&expected_stdout)
}

/// Ensures the reported version matches the crate metadata.
#[then("stdout contains the hello world version")]
pub fn stdout_contains_version(world: &mut World) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let expected = format!("hello-world {version}");
    world.assert_stdout_contains(&expected)
}

#[then(expr = "stderr contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
// Step captures arrive as owned `String` values from cucumber; forward them
// to the world helper so assertions can consume the owned capture directly.
pub fn stderr_contains(world: &mut World, expected_stderr: String) -> Result<()> {
    world.assert_stderr_contains(&expected_stderr)
}

#[given(expr = "the environment contains {string} = {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
pub fn environment_contains(world: &mut World, env_key: String, env_value: String) -> Result<()> {
    validate_env_key(&env_key)?;
    world.set_env(&env_key, &env_value);
    Ok(())
}

#[given(expr = "the environment does not contain {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
pub fn environment_does_not_contain(world: &mut World, env_key: String) -> Result<()> {
    validate_env_key(&env_key)?;
    world.remove_env(&env_key);
    Ok(())
}

/// Writes docstring contents to the default configuration file.
#[given("the hello world config file contains:")]
pub fn config_file(world: &mut World, step: &GherkinStep) -> Result<()> {
    let contents = extract_docstring(step)?;
    world.write_config(contents)
}

/// Writes docstring contents to a named file.
#[given(expr = "the file {string} contains:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
pub fn named_file_contains(world: &mut World, name: String, step: &GherkinStep) -> Result<()> {
    let contents = extract_docstring(step)?;
    world.write_named_file(&name, contents)
}

/// Writes docstring contents to the XDG config home directory.
#[given("the XDG config home contains:")]
pub fn xdg_config_home_contains(world: &mut World, step: &GherkinStep) -> Result<()> {
    let contents = extract_docstring(step)?;
    world.write_xdg_config_home(contents)
}

/// Initialises the scenario using a repository sample configuration.
#[given(expr = "I start from the sample hello world config {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
pub fn start_from_sample_config(world: &mut World, sample: String) -> Result<()> {
    world.write_sample_config(&sample)
}

#[given(expr = "I start from a missing or invalid sample config {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
pub fn start_from_invalid_sample_config(world: &mut World, sample_name: String) -> Result<()> {
    match world.try_write_sample_config(&sample_name) {
        Ok(()) => {
            return Err(anyhow!(
                "expected sample config {sample_name:?} to be missing or invalid"
            ));
        }
        Err(
            SampleConfigError::OpenSample { .. }
            | SampleConfigError::ReadSample { .. }
            | SampleConfigError::WriteSample { .. },
        ) => {}
        Err(err) => return Err(anyhow!("unexpected sample config error: {err}")),
    }
    Ok(())
}

fn compose_declarative_globals_from_contents(world: &mut World, contents: &str) -> Result<()> {
    let inputs: Vec<LayerInput> =
        serde_json::from_str(contents).context("valid JSON describing declarative layers")?;
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
            other => return Err(anyhow!("unknown provenance {other}")),
        }
    }
    let globals = GlobalArgs::merge_from_layers(composer.layers())
        .context("declarative merge should succeed for globals")?;
    world.set_declarative_globals(globals);
    Ok(())
}

#[given("I compose hello world globals from declarative layers:")]
pub fn compose_declarative_globals(world: &mut World, step: &GherkinStep) -> Result<()> {
    let contents = extract_docstring(step)?;
    compose_declarative_globals_from_contents(world, contents)
}

#[then(expr = "the declarative globals recipient is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned capture values"
)]
pub fn assert_declarative_recipient(world: &mut World, expected: String) -> Result<()> {
    world.assert_declarative_recipient(&expected)
}

#[then("the declarative globals salutations are:")]
pub fn assert_declarative_salutations(world: &mut World, step: &GherkinStep) -> Result<()> {
    let contents = extract_docstring(step)?;
    let expected: Vec<String> = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    world.assert_declarative_salutations(&expected)
}

#[cfg(test)]
mod tests;
