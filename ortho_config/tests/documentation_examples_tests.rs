//! Executable contracts for examples in public documentation.

mod documentation_examples;

#[path = "documentation_examples/format_all.rs"]
mod format_all;

#[path = "documentation_examples/static_contracts.rs"]
mod static_contracts;

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs::Dir};
use documentation_examples::{documented_example, load_documented_examples};
use ortho_config::{AgentContext, toml};
use std::collections::BTreeSet;

const EXPECTED_EXAMPLE_IDS: &[&str] = &[
    "api-guide-agent-context-constructor",
    "api-guide-agent-context-json",
    "api-guide-policy-report-constructors",
    "api-guide-policy-report-json",
    "guide-agent-context",
    "guide-alias-derive",
    "guide-alias-install",
    "guide-behaviour-metadata",
    "guide-boolean-override",
    "guide-cargo-error-hints",
    "guide-cargo-external-subcommand",
    "guide-check-command",
    "guide-check-report",
    "guide-clap-only-parse",
    "guide-collection-file",
    "guide-collections",
    "guide-compose-layers-with-sources",
    "guide-csv-env-with-source",
    "guide-discovery",
    "guide-errors",
    "guide-file",
    "guide-file-run",
    "guide-first-cli",
    "guide-hermetic-discovery",
    "guide-identifier-artefact",
    "guide-install",
    "guide-load-first-outcomes",
    "guide-localization",
    "guide-localization-ids",
    "guide-localization-ids-output",
    "guide-metrics-install",
    "guide-orthohelp-command",
    "guide-orthohelp-format-all",
    "guide-orthohelp-format-all-tree",
    "guide-orthohelp-metadata",
    "guide-policy-check",
    "guide-policy-ci",
    "guide-policy-config",
    "guide-profile-metadata",
    "guide-profiles",
    "guide-profiles-file",
    "guide-profiles-run",
    "guide-scoped-discovery",
    "guide-selected-subcommand-sources",
    "guide-source-aware-load",
    "guide-subcommand",
    "guide-subcommand-merge-sources",
    "guide-subcommand-sources",
    "guide-tracing",
    "guide-tracing-install",
    "guide-yaml",
    "guide-yaml-install",
    "migration-agent-context-profiles",
    "migration-boolean-file",
    "migration-boolean-run",
    "migration-cli-default-after",
    "migration-cli-default-before",
    "migration-cli-default-run",
    "migration-dependencies-after",
    "migration-dependencies-before",
    "migration-explicit-cli",
    "migration-explicit-cli-run",
    "migration-l10n-dependency",
    "migration-l10n-runtime",
    "migration-profile-precedence",
    "readme-binstall",
    "readme-install",
    "readme-main",
    "readme-run",
];

#[test]
fn published_crate_readme_matches_repository_readme() -> Result<()> {
    let repository = Dir::open_ambient_dir(repository_root().as_std_path(), ambient_authority())?;
    let repository_readme = repository.read_to_string("README.md")?;
    let crate_readme = repository.read_to_string("ortho_config/README.md")?;
    ensure!(
        crate_readme == repository_readme,
        "the packaged ortho_config README should match the repository README"
    );
    Ok(())
}

#[test]
fn every_documented_fence_has_a_known_unique_identifier() -> Result<()> {
    let examples = load_documented_examples()?;
    let actual = examples
        .iter()
        .map(|example| example.id.as_str())
        .collect::<BTreeSet<_>>();
    let expected = EXPECTED_EXAMPLE_IDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    ensure!(
        actual == expected,
        "documented example registry drifted\nexpected: {expected:#?}\nactual: {actual:#?}"
    );
    Ok(())
}

#[test]
fn configuration_files_deserialize_with_the_documented_shapes() -> Result<()> {
    let file = parse_toml("guide-file")?;
    ensure!(file.get("host").and_then(toml::Value::as_str) == Some("0.0.0.0"));
    ensure!(file.get("port").and_then(toml::Value::as_integer) == Some(9000));
    ensure!(file.get("log_level").and_then(toml::Value::as_str) == Some("debug"));

    let collections = parse_toml("guide-collection-file")?;
    let workers = collections
        .get("workers")
        .context("workers should exist")?
        .as_array()
        .context("workers should be an array of tables")?;
    ensure!(workers.len() == 2);
    let first_worker = workers.first().context("first worker should exist")?;
    let second_worker = workers.get(1).context("second worker should exist")?;
    ensure!(first_worker.get("name").and_then(toml::Value::as_str) == Some("queue-a"));
    ensure!(
        second_worker
            .get("concurrency")
            .and_then(toml::Value::as_integer)
            == Some(2)
    );
    let labels = collections.get("labels").context("labels should exist")?;
    ensure!(labels.get("region").and_then(toml::Value::as_str) == Some("eu-west"));
    Ok(())
}

#[test]
fn agent_context_json_matches_the_runtime_default() -> Result<()> {
    let example = documented_example("guide-agent-context")?;
    ensure!(example.language == "json");
    let documented: AgentContext = ortho_config::serde_json::from_str(&example.body)?;
    ensure!(documented == AgentContext::new("acme"));
    Ok(())
}

#[cfg(feature = "yaml")]
#[test]
fn yaml_example_uses_yaml_1_2_string_semantics() -> Result<()> {
    let example = documented_example("guide-yaml")?;
    ensure!(example.language == "yaml");
    let provider = ortho_config::file::SaphyrYaml::string("guide.yaml", &example.body);
    let value = ortho_config::figment::Figment::from(provider)
        .extract::<ortho_config::serde_json::Value>()?;
    ensure!(
        value
            .get("enabled")
            .and_then(ortho_config::serde_json::Value::as_str)
            == Some("yes")
    );
    ensure!(
        value
            .get("mode")
            .and_then(ortho_config::serde_json::Value::as_str)
            == Some("on")
    );
    ensure!(
        value
            .get("port")
            .and_then(ortho_config::serde_json::Value::as_u64)
            == Some(8080)
    );
    Ok(())
}

#[test]
fn documented_orthohelp_command_matches_cli_contract() -> Result<()> {
    let example = documented_example("guide-orthohelp-command")?;
    ensure!(example.language == "console");
    ensure!(
        example.body == "cargo orthohelp --package hello_world --format agent-context\n",
        "cargo-orthohelp command contract drifted"
    );

    Ok(())
}

fn parse_toml(id: &str) -> Result<toml::Value> {
    let example = documented_example(id)?;
    ensure!(example.language == "toml", "{id} should be TOML");
    toml::from_str(&example.body).with_context(|| format!("parse {id}"))
}

fn repository_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
