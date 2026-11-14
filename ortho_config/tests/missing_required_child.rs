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
    let executable = std::env::current_exe().context("locate the test executable")?;
    let mut command = Command::new(executable);
    command
        .args([
            "--ignored",
            "--exact",
            "--nocapture",
            "--test-threads=1",
            PROBE,
        ])
        .env_clear()
        .env(MODE_VAR, mode);
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
