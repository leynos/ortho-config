//! Child-process evidence for a consumer's missing-value exit policy.
//!
//! The library never exits; a consumer decides what to do with the error. The
//! probe below plays that consumer in a fresh process: it loads a
//! configuration from its own arguments and environment, prints the rendered
//! diagnostic on standard error, and exits non-zero. The parent supplies every
//! variable through `Command::env`, so the test process environment is never
//! mutated, and it asserts both the failure and the recovery for each
//! advertised route.

use anyhow::{Context as _, Result, anyhow};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::OrthoConfig;
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The ignored probe test the parent invokes by exact name.
const PROBE: &str = "consumer_probe";
/// Selects which inputs the probe supplies.
const MODE_VAR: &str = "MRQ_CHILD_MODE";
/// Points discovery at the fixture configuration file.
const CONFIG_PATH_VAR: &str = "MRQ_CHILD_CONFIG_PATH";
/// Marks a successful load on standard output.
const SUCCESS_MARKER: &str = "MRQ_CHILD_LOADED";

/// A consumer configuration with two independent required values.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRQ_CHILD_")]
struct ConsumerConfig {
    /// Required scalar reachable from every route.
    reference: String,
    /// Required scalar proving the report enumerates more than one value.
    region: String,
}

/// Run the probe, reporting only the diagnostic a consumer would print.
#[test]
#[ignore = "run by the parent tests with an isolated child environment"]
fn consumer_probe() -> Result<()> {
    if let Err(error) = run_consumer() {
        writeln!(std::io::stderr().lock(), "error: {error}")?;
        std::process::exit(1);
    }
    Ok(())
}

/// Load the configuration and report it as a consumer would.
fn run_consumer() -> Result<()> {
    let config =
        ConsumerConfig::load_from_iter(probe_args()).map_err(|error| anyhow!("{error}"))?;
    writeln!(
        std::io::stdout().lock(),
        "{SUCCESS_MARKER} reference={} region={}",
        config.reference,
        config.region
    )?;
    Ok(())
}

/// Build the arguments the probe supplies to the loader.
fn probe_args() -> Vec<String> {
    let mode = std::env::var(MODE_VAR).unwrap_or_default();
    let mut args = vec![String::from("consumer")];
    if matches!(mode.as_str(), "cli" | "mixed") {
        args.extend([String::from("--reference"), String::from("cli-ref")]);
    }
    if mode == "cli" {
        args.extend([String::from("--region"), String::from("cli-region")]);
    }
    args
}

/// Write `contents` into a fresh capability-based fixture file.
fn write_config(directory: &Path, contents: &str) -> Result<PathBuf> {
    let cap = Dir::open_ambient_dir(directory, ambient_authority())
        .context("open the child fixture directory")?;
    cap.write("consumer.toml", contents.as_bytes())
        .context("write the child fixture file")?;
    Ok(directory.join("consumer.toml"))
}

/// Build one name/value pair for the probe's environment.
fn var<'a>(name: &'a str, value: &'a str) -> (&'a OsStr, &'a OsStr) {
    (OsStr::new(name), OsStr::new(value))
}

/// Build the pair that points discovery at `path`.
fn config_var(path: &Path) -> (&OsStr, &OsStr) {
    (OsStr::new(CONFIG_PATH_VAR), path.as_os_str())
}

/// Run the probe in a fresh environment containing only `variables`.
fn run_probe(mode: &str, variables: &[(&OsStr, &OsStr)]) -> Result<Output> {
    let mut all = vec![(OsStr::new(MODE_VAR), OsStr::new(mode))];
    all.extend_from_slice(variables);
    run_ignored(PROBE, &all)
}

/// Run one ignored probe test in a fresh environment containing only
/// `variables`.
///
/// `Command::env` scopes every variable to the child, so the test process
/// environment is never mutated and the probe sees exactly what a consumer
/// would see from the platform.
fn run_ignored(probe: &str, variables: &[(&OsStr, &OsStr)]) -> Result<Output> {
    let executable = std::env::current_exe().context("locate the test executable")?;
    let mut command = Command::new(executable);
    command
        .args([
            "--ignored",
            "--exact",
            "--nocapture",
            "--test-threads=1",
            probe,
        ])
        .env_clear();
    for (key, value) in variables {
        command.env(key, value);
    }
    command.output().context("run the consumer probe")
}

/// Decode both captured streams for assertions.
fn streams(output: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Assert the child failed with the enumerated diagnostic and no success line.
fn assert_missing_diagnostic(output: &Output) {
    let (stdout, stderr) = streams(output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected the consumer's exit code for a failed load: stdout={stdout} stderr={stderr}"
    );
    assert!(
        !stdout.contains(SUCCESS_MARKER),
        "a failed load must not print a success line: {stdout}"
    );
    assert!(
        stderr.contains("error: Missing required configuration for `consumer`"),
        "expected the rendered diagnostic on stderr: {stderr}"
    );
    assert!(
        stderr.contains("`reference`"),
        "missing reference: {stderr}"
    );
    assert!(stderr.contains("`region`"), "missing region: {stderr}");
    assert!(
        stderr.contains("--reference <value>"),
        "missing the command-line route: {stderr}"
    );
    assert!(
        stderr.contains("MRQ_CHILD_REFERENCE=<value>"),
        "missing the environment route: {stderr}"
    );
}

/// Assert the child loaded the values without printing a diagnostic.
fn assert_loaded(output: &Output, reference: &str, region: &str) {
    let (stdout, stderr) = streams(output);
    assert!(
        output.status.success(),
        "expected a successful load: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains(&format!(
            "{SUCCESS_MARKER} reference={reference} region={region}"
        )),
        "expected the loaded values on stdout: {stdout}"
    );
    assert!(
        !stderr.contains("Missing required configuration"),
        "a recovery must not print a diagnostic: {stderr}"
    );
}

#[test]
fn an_unsupplied_configuration_fails_with_a_diagnostic() -> Result<()> {
    let directory = tempfile::tempdir().context("create the child fixture directory")?;
    let path = write_config(directory.path(), "")?;
    let output = run_probe("fail", &[config_var(&path)])?;
    assert_missing_diagnostic(&output);
    Ok(())
}

/// One advertised route's child-process fixture.
#[derive(Clone, Copy)]
struct SupplyCase {
    /// Inputs the probe supplies, as understood by `probe_args`.
    mode: &'static str,
    /// Contents of the discovery file the probe reads.
    file: &'static str,
    /// Extra environment pairs, beyond the discovery path every case sets.
    variables: &'static [(&'static str, &'static str)],
    /// Value the probe must load for `reference`.
    reference: &'static str,
    /// Value the probe must load for `region`.
    region: &'static str,
}

#[rstest]
#[case::command_line(SupplyCase {
    mode: "cli",
    file: "",
    variables: &[],
    reference: "cli-ref",
    region: "cli-region",
})]
#[case::environment(SupplyCase {
    mode: "env",
    file: "",
    variables: &[("MRQ_CHILD_REFERENCE", "env-ref"), ("MRQ_CHILD_REGION", "env-region")],
    reference: "env-ref",
    region: "env-region",
})]
#[case::file(SupplyCase {
    mode: "file",
    file: "reference = \"file-ref\"\nregion = \"file-region\"\n",
    variables: &[],
    reference: "file-ref",
    region: "file-region",
})]
#[case::mixed(SupplyCase {
    mode: "mixed",
    file: "",
    variables: &[("MRQ_CHILD_REGION", "env-region")],
    reference: "cli-ref",
    region: "env-region",
})]
fn each_advertised_route_recovers(#[case] case: SupplyCase) -> Result<()> {
    let directory = tempfile::tempdir().context("create the child fixture directory")?;
    let path = write_config(directory.path(), case.file)?;
    let mut variables = vec![config_var(&path)];
    variables.extend(case.variables.iter().map(|(name, value)| var(name, value)));
    let output = run_probe(case.mode, &variables)?;
    assert_loaded(&output, case.reference, case.region);
    Ok(())
}

/// Points the equality probe at its discovery file.
const EQUALS_CONFIG_PATH_VAR: &str = "MREQ_CONFIG_PATH";
/// The ignored equality probe the parent invokes by exact name.
const EQUALS_PROBE: &str = "equals_key_probe";
/// Marks a successful load by the equality probe.
const EQUALS_SUCCESS_MARKER: &str = "MREQ_CHILD_LOADED";

/// A required leaf the quoted table supplies.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct EqualsDatabase {
    /// The leaf the process-backed report must name.
    host: String,
    /// Optional leaf used to supply the object only partially.
    #[serde(default)]
    port: u16,
}

/// A consumer whose alias spelling no process variable can carry.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MREQ_")]
struct EqualsConsumer {
    /// The alias is observed from a quoted table; `=` separates a variable's
    /// name from its value, so no process environment can express it.
    #[serde(alias = "Host=Name")]
    #[ortho_config(nested, skip_cli)]
    database: EqualsDatabase,
}

/// Load the equality consumer from the child's own environment and file.
#[test]
#[ignore = "run by the parent test with an isolated child environment"]
fn equals_key_probe() -> Result<()> {
    if let Err(error) = run_equals_consumer() {
        writeln!(std::io::stderr().lock(), "error: {error}")?;
        std::process::exit(1);
    }
    Ok(())
}

/// Report the equality consumer's values as a consumer would.
fn run_equals_consumer() -> Result<()> {
    let config = EqualsConsumer::load_from_iter([String::from("consumer")])
        .map_err(|error| anyhow!("{error}"))?;
    writeln!(
        std::io::stdout().lock(),
        "{EQUALS_SUCCESS_MARKER} host={} port={}",
        config.database.host,
        config.database.port
    )?;
    Ok(())
}

/// A `=` key is unreachable from a real process environment.
///
/// The probe reads the platform environment, so this is the process-backed
/// counterpart of the injected `MRV_` case in
/// `support/missing_required_supply.rs`. The report must omit the environment
/// section entirely and keep only the quoted file route, and that route must
/// then recover the value in the same process-backed load.
#[test]
fn a_process_environment_cannot_carry_an_equals_key() -> Result<()> {
    let directory = tempfile::tempdir().context("create the child fixture directory")?;
    let partial = write_config(directory.path(), "[\"Host=Name\"]\nport = 1\n")?;
    let output = run_ignored(
        EQUALS_PROBE,
        &[(OsStr::new(EQUALS_CONFIG_PATH_VAR), partial.as_os_str())],
    )?;
    assert_equals_report_is_file_only(&output);

    let recovery = tempfile::tempdir().context("create the recovery directory")?;
    let supplied = write_config(
        recovery.path(),
        "[\"Host=Name\"]\nhost = \"db\"\nport = 2\n",
    )?;
    let recovered = run_ignored(
        EQUALS_PROBE,
        &[(OsStr::new(EQUALS_CONFIG_PATH_VAR), supplied.as_os_str())],
    )?;
    assert_equals_recovery(&recovered);
    Ok(())
}

/// Assert the process-backed report offers only the quoted file route.
fn assert_equals_report_is_file_only(output: &Output) {
    let (stdout, stderr) = streams(output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected the consumer's exit code for a failed load: stdout={stdout} stderr={stderr}"
    );
    assert!(
        !stdout.contains(EQUALS_SUCCESS_MARKER),
        "a failed load must not print a success line: {stdout}"
    );
    assert!(
        stderr.contains("`database.host`"),
        "the leaf must be reported: {stderr}"
    );
    assert!(
        !stderr.contains("Environment Variable"),
        "no process variable can carry a `=` key: {stderr}"
    );
    assert!(
        !stderr.contains("MREQ_DATABASE__HOST"),
        "an impossible variable must never be advertised: {stderr}"
    );
    assert!(
        stderr.contains(r#"["Host=Name"]"#) && stderr.contains(r#"host = "<value>""#),
        "the file route must quote the observed spelling as a table header: {stderr}"
    );
}

/// Assert the quoted table supplied both leaves.
fn assert_equals_recovery(output: &Output) {
    let (stdout, stderr) = streams(output);
    assert!(
        output.status.success(),
        "the advertised file route must recover: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains(&format!("{EQUALS_SUCCESS_MARKER} host=db port=2")),
        "the quoted table must supply both leaves: {stdout}"
    );
}
