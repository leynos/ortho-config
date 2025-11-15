//! Step definitions for the `hello_world` example.
//! Drive the binary and assert its observable outputs.
#![expect(
    clippy::shadow_reuse,
    reason = "rstest-bdd step macros rebind placeholders during expansion"
)]

use crate::behaviour::config::SampleConfigError;
use crate::behaviour::harness::Harness;
use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use hello_world::cli::GlobalArgs;
use ortho_config::serde_json::{self, Value};
use ortho_config::MergeComposer;
use rstest_bdd_macros::{given, then, when};
use serde::Deserialize;

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
pub fn run_without_args(#[from(hello_world_harness)] harness: &mut Harness) -> Result<()> {
    harness.run_hello(None)
}

#[when("I run the hello world example with arguments {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd step macros require owned capture values"
)]
pub fn run_with_args(
    #[from(hello_world_harness)] harness: &mut Harness,
    arguments: String,
) -> Result<()> {
    harness.run_hello(Some(arguments))
}

#[then("the command succeeds")]
pub fn command_succeeds(#[from(hello_world_harness)] harness: &mut Harness) -> Result<()> {
    harness.assert_success()
}

#[then("the command fails")]
pub fn command_fails(#[from(hello_world_harness)] harness: &mut Harness) -> Result<()> {
    harness.assert_failure()
}

#[then("stdout contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd step macros require owned capture values"
)]
pub fn stdout_contains(
    #[from(hello_world_harness)] harness: &mut Harness,
    expected_stdout: String,
) -> Result<()> {
    harness.assert_stdout_contains(&expected_stdout)
}

/// Ensures the reported version matches the crate metadata.
#[then("stdout contains the hello world version")]
pub fn stdout_contains_version(
    #[from(hello_world_harness)] harness: &mut Harness,
) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let expected = format!("hello-world {version}");
    harness.assert_stdout_contains(&expected)
}

#[then("stderr contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd step macros require owned capture values"
)]
pub fn stderr_contains(
    #[from(hello_world_harness)] harness: &mut Harness,
    expected_stderr: String,
) -> Result<()> {
    harness.assert_stderr_contains(&expected_stderr)
}

#[given("the environment contains {string} = {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd step macros require owned capture values"
)]
pub fn environment_contains(
    #[from(hello_world_harness)] harness: &mut Harness,
    env_key: String,
    env_value: String,
) -> Result<()> {
    validate_env_key(&env_key)?;
    harness.set_env(&env_key, &env_value);
    Ok(())
}

#[given("the environment does not contain {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd step macros require owned capture values"
)]
pub fn environment_does_not_contain(
    #[from(hello_world_harness)] harness: &mut Harness,
    env_key: String,
) -> Result<()> {
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
#[given("the file {string} contains:")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd step macros require owned capture values"
)]
pub fn named_file_contains(
    #[from(hello_world_harness)] harness: &Harness,
    name: String,
    docstring: String,
) -> Result<()> {
    harness.write_named_file(&name, &docstring)
}

/// Writes docstring contents to the XDG config home directory.
#[given("the XDG config home contains:")]
pub fn xdg_config_home_contains(
    #[from(hello_world_harness)] harness: &Harness,
    docstring: String,
) -> Result<()> {
    harness.write_xdg_config_home(&docstring)
}

/// Initialises the scenario using a repository sample configuration.
#[given("I start from the sample hello world config {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd step macros require owned capture values"
)]
pub fn start_from_sample_config(
    #[from(hello_world_harness)] harness: &Harness,
    sample: String,
) -> Result<()> {
    harness.write_sample_config(&sample)
}

#[given("I start from a missing or invalid sample config {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd step macros require owned capture values"
)]
pub fn start_from_invalid_sample_config(
    #[from(hello_world_harness)] harness: &Harness,
    sample_name: String,
) -> Result<()> {
    match harness.try_write_sample_config(&sample_name) {
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

fn compose_declarative_globals_from_contents(
    harness: &mut Harness,
    contents: &str,
) -> Result<()> {
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

#[then("the declarative globals recipient is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "rstest-bdd step macros require owned capture values"
)]
pub fn assert_declarative_recipient(
    #[from(hello_world_harness)] harness: &Harness,
    expected: String,
) -> Result<()> {
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
mod tests;
EOF
