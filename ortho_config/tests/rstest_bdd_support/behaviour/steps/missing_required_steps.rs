//! Steps for missing required value diagnostics and recovery.
//!
//! Every load goes through the generated loader with injected sources, so the
//! scenarios never mutate the test process environment. A configuration file
//! is written into a fresh capability-based directory for each load, and the
//! directory is dropped once the load has read it.
//!
//! The scenarios cover one-diagnostic recovery through the command line and
//! through a file-plus-environment mix, confirm optional and supplied values
//! are not reported, and keep an invalid nested value distinct from an absent
//! one.

use super::common::{SlotTakeOrExt, config_fixture, shared_sources};
use super::value_parsing::normalize_scalar;
use anyhow::{Result, anyhow, ensure};
use ortho_config::{MapEnv, OrthoConfig, OrthoError};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, then, when};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A nested object whose only leaf is required.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct DatabaseSettings {
    /// Required leaf inside the nested object.
    host: String,
}

/// A configuration whose required value is a top-level leaf.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRQB_")]
struct ReferenceConfig {
    /// Required scalar with no default, so the parser offers a flag for it.
    reference: String,
    /// Optional subtree: an absent object deactivates its children.
    #[ortho_config(nested, skip_cli)]
    database: Option<DatabaseSettings>,
    /// Optional because an `ortho_config` default exists.
    #[ortho_config(default = 8080)]
    port: u16,
    /// Optional because the type is `Option`.
    note: Option<String>,
}

/// A configuration whose only required value is nested.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRQN_")]
struct NestedConfig {
    /// No flag can carry an object; the leaf is reachable from the other routes.
    #[ortho_config(nested, skip_cli)]
    database: DatabaseSettings,
}

/// Scenario state for missing required value diagnostics.
#[derive(Default, ScenarioState)]
pub struct MissingRequiredContext {
    file_contents: Slot<String>,
    reference_result: Slot<ortho_config::OrthoResult<ReferenceConfig>>,
    nested_result: Slot<ortho_config::OrthoResult<NestedConfig>>,
}

/// Provides a clean context for missing required value scenarios.
#[fixture]
pub fn missing_required_context() -> MissingRequiredContext {
    MissingRequiredContext::default()
}

/// Build an environment carrying the fixture file's path and `assignments`.
fn injected_environment(path: &Path, assignments: &[(&str, &str)]) -> MapEnv {
    let mut environment = MapEnv::new().with_var("MRQB_CONFIG_PATH", path);
    for (key, value) in assignments {
        environment = environment.with_var(*key, *value);
    }
    environment
}

/// Load [`ReferenceConfig`] from a fixture file plus explicit values.
fn load_reference(
    missing_required_context: &MissingRequiredContext,
    args: &[&str],
    assignments: &[(&str, &str)],
) -> Result<ortho_config::OrthoResult<ReferenceConfig>> {
    let contents = missing_required_context
        .file_contents
        .get()
        .unwrap_or_default();
    let (_directory, path) = config_fixture("mrq.toml", Some(contents.as_str()))?;
    let environment = injected_environment(&path, assignments);
    let (discovery, merge) = shared_sources(environment);
    let mut command_line = vec!["consumer"];
    command_line.extend_from_slice(args);
    Ok(ReferenceConfig::load_from_iter_with_sources(
        command_line,
        discovery,
        merge,
    ))
}

/// Load [`NestedConfig`] from a fixture file plus explicit values.
fn load_nested(
    missing_required_context: &MissingRequiredContext,
    assignments: &[(&str, &str)],
) -> Result<ortho_config::OrthoResult<NestedConfig>> {
    let contents = missing_required_context
        .file_contents
        .get()
        .unwrap_or_default();
    let (_directory, path) = config_fixture("mrqn.toml", Some(contents.as_str()))?;
    let mut environment = MapEnv::new().with_var("MRQN_CONFIG_PATH", path);
    for (key, value) in assignments {
        environment = environment.with_var(*key, *value);
    }
    let (discovery, merge) = shared_sources(environment);
    Ok(NestedConfig::load_from_iter_with_sources(
        ["consumer"],
        discovery,
        merge,
    ))
}

/// Extract the report from a failed load, rejecting other error kinds.
fn missing_report(error: &OrthoError) -> Result<&ortho_config::MissingRequiredValues> {
    match error {
        OrthoError::MissingRequiredValues(report) => Ok(report.as_ref()),
        other => Err(anyhow!("expected missing values, got: {other}")),
    }
}

impl MissingRequiredContext {
    /// Take the reference load recorded by a `when` step.
    fn take_reference(&self) -> Result<ortho_config::OrthoResult<ReferenceConfig>> {
        self.reference_result
            .take_or("reference load result unavailable")
    }

    /// Take the nested load recorded by a `when` step.
    fn take_nested(&self) -> Result<ortho_config::OrthoResult<NestedConfig>> {
        self.nested_result.take_or("nested load result unavailable")
    }

    /// Take the reference load, or repeat the load when an earlier assertion
    /// already consumed it.
    fn reference_or_reload(&self) -> Result<ortho_config::OrthoResult<ReferenceConfig>> {
        match self.reference_result.take() {
            Some(result) => Ok(result),
            None => load_reference(self, &[], &[]),
        }
    }

    /// Take the nested load, or repeat the load when an earlier assertion
    /// already consumed it.
    fn nested_or_reload(&self) -> Result<ortho_config::OrthoResult<NestedConfig>> {
        match self.nested_result.take() {
            Some(result) => Ok(result),
            None => load_nested(self, &[]),
        }
    }
}

/// Convert a successful load into its configuration, rendering any failure.
fn loaded_config<T>(result: ortho_config::OrthoResult<T>) -> Result<T> {
    result.map_err(|error| anyhow!("{error}"))
}

/// Assert `result` failed with exactly one diagnostic naming `path`.
fn assert_single_missing_path<T>(result: ortho_config::OrthoResult<T>, path: &str) -> Result<()> {
    let error = result.err().ok_or_else(|| anyhow!("expected a failure"))?;
    let report = missing_report(error.as_ref())?;
    ensure!(
        report.len() == 1,
        "expected one diagnostic, got {}: {}",
        report.len(),
        report
    );
    let actual = report
        .items()
        .first()
        .map(|item| item.path().render())
        .unwrap_or_default();
    ensure!(
        actual == normalize_scalar(path),
        "expected the path {}, got {actual}",
        normalize_scalar(path)
    );
    Ok(())
}

/// Assert the diagnostic from `result` advertises `needle` as its `subject`.
fn assert_diagnostic_mentions<T>(
    result: ortho_config::OrthoResult<T>,
    needle: &str,
    subject: &str,
) -> Result<()> {
    let error = result.err().ok_or_else(|| anyhow!("expected a failure"))?;
    let rendered = error.to_string();
    ensure!(
        rendered.contains(&normalize_scalar(needle)),
        "expected the {subject} in the diagnostic: {rendered}"
    );
    Ok(())
}

#[given("a configuration requiring a reference")]
fn configuration_requiring_reference(
    missing_required_context: &MissingRequiredContext,
) -> Result<()> {
    missing_required_context.file_contents.set(String::new());
    Ok(())
}

#[when("the configuration is loaded without the required value")]
fn load_without_reference(missing_required_context: &MissingRequiredContext) -> Result<()> {
    let result = load_reference(missing_required_context, &[], &[])?;
    missing_required_context.reference_result.set(result);
    Ok(())
}

#[then("exactly one diagnostic names the path {path}")]
fn one_diagnostic_names_path(
    missing_required_context: &MissingRequiredContext,
    path: String,
) -> Result<()> {
    assert_single_missing_path(missing_required_context.take_reference()?, &path)
}

#[then("the diagnostic advertises the command-line flag {flag}")]
fn diagnostic_advertises_flag(
    missing_required_context: &MissingRequiredContext,
    flag: String,
) -> Result<()> {
    assert_diagnostic_mentions(
        missing_required_context.reference_or_reload()?,
        &flag,
        "flag",
    )
}

#[when("the reference is supplied on the command line")]
fn supply_reference_on_command_line(
    missing_required_context: &MissingRequiredContext,
) -> Result<()> {
    let result = load_reference(missing_required_context, &["--reference", "r-1"], &[])?;
    missing_required_context.reference_result.set(result);
    Ok(())
}

#[then("the configuration loads successfully")]
fn configuration_loads(missing_required_context: &MissingRequiredContext) -> Result<()> {
    let config = loaded_config(missing_required_context.take_reference()?)?;
    ensure!(config.reference == "r-1", "unexpected reference value");
    Ok(())
}

#[when("the configuration is loaded with a supplied reference and an absent optional value")]
fn load_with_reference_and_absent_optional(
    missing_required_context: &MissingRequiredContext,
) -> Result<()> {
    let result = load_reference(missing_required_context, &["--reference", "r-1"], &[])?;
    missing_required_context.reference_result.set(result);
    Ok(())
}

#[then("no missing-value diagnostic is produced")]
fn no_missing_diagnostic(missing_required_context: &MissingRequiredContext) -> Result<()> {
    let result = missing_required_context.take_reference()?;
    ensure!(
        result.is_ok(),
        "expected a successful load, got {}",
        result
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default()
    );
    Ok(())
}

#[then("the configured default port is applied")]
fn default_port_applied(missing_required_context: &MissingRequiredContext) -> Result<()> {
    // The successful load was consumed by the preceding assertion, so reload the
    // same inputs here rather than sharing a non-cloneable configuration.
    let result = load_reference(missing_required_context, &["--reference", "r-1"], &[])?;
    let config = loaded_config(result)?;
    ensure!(
        config.port == 8080,
        "expected the default port, got {}",
        config.port
    );
    ensure!(config.note.is_none(), "the optional note must stay absent");
    ensure!(
        config.database.is_none(),
        "the optional subtree must stay absent"
    );
    Ok(())
}

#[given("a nested configuration with an empty database table")]
fn nested_configuration_with_empty_table(
    missing_required_context: &MissingRequiredContext,
) -> Result<()> {
    missing_required_context
        .file_contents
        .set(String::from("[database]\n"));
    Ok(())
}

#[when("the nested configuration is loaded")]
fn load_nested_configuration(missing_required_context: &MissingRequiredContext) -> Result<()> {
    let result = load_nested(missing_required_context, &[])?;
    missing_required_context.nested_result.set(result);
    Ok(())
}

#[then("exactly one nested diagnostic names the path {path}")]
fn one_nested_diagnostic_names_path(
    missing_required_context: &MissingRequiredContext,
    path: String,
) -> Result<()> {
    assert_single_missing_path(missing_required_context.take_nested()?, &path)
}

#[then("the nested diagnostic advertises the environment variable {variable}")]
fn nested_diagnostic_advertises_variable(
    missing_required_context: &MissingRequiredContext,
    variable: String,
) -> Result<()> {
    assert_diagnostic_mentions(
        missing_required_context.nested_or_reload()?,
        &variable,
        "variable",
    )
}

#[when("the database host is supplied through the environment")]
fn supply_database_host(missing_required_context: &MissingRequiredContext) -> Result<()> {
    let result = load_nested(
        missing_required_context,
        &[("MRQN_DATABASE__HOST", "db.example.test")],
    )?;
    missing_required_context.nested_result.set(result);
    Ok(())
}

#[then("the nested configuration loads successfully")]
fn nested_configuration_loads(missing_required_context: &MissingRequiredContext) -> Result<()> {
    let config = loaded_config(missing_required_context.take_nested()?)?;
    ensure!(
        config.database.host == "db.example.test",
        "unexpected database host"
    );
    Ok(())
}

#[given("a nested configuration whose database value is not an object")]
fn nested_configuration_with_scalar_database(
    missing_required_context: &MissingRequiredContext,
) -> Result<()> {
    missing_required_context
        .file_contents
        .set(String::from("database = \"not-an-object\"\n"));
    Ok(())
}

#[then("a configuration error is returned instead of a missing-value diagnostic")]
fn configuration_error_not_missing(
    missing_required_context: &MissingRequiredContext,
) -> Result<()> {
    let result = missing_required_context.take_nested()?;
    let error = result
        .err()
        .ok_or_else(|| anyhow!("expected a deserialization failure"))?;
    ensure!(
        !matches!(error.as_ref(), OrthoError::MissingRequiredValues(_)),
        "an invalid value must not be reported as absence: {error}"
    );
    Ok(())
}
