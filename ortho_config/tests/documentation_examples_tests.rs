//! Executable contracts for examples in the README and user's guide.

mod documentation_examples;

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use documentation_examples::{documented_example, load_documented_examples};
use ortho_config::{AgentContext, toml};
use std::collections::BTreeSet;
use std::process::Command;
use tempfile::TempDir;

const EXPECTED_EXAMPLE_IDS: &[&str] = &[
    "guide-agent-context",
    "guide-alias-derive",
    "guide-alias-install",
    "guide-collection-file",
    "guide-discovery",
    "guide-errors",
    "guide-file",
    "guide-file-run",
    "guide-first-cli",
    "guide-hermetic-discovery",
    "guide-install",
    "guide-load-first-outcomes",
    "guide-localization",
    "guide-metrics-install",
    "guide-orthohelp-command",
    "guide-orthohelp-metadata",
    "guide-subcommand",
    "guide-tracing",
    "guide-tracing-install",
    "guide-yaml",
    "readme-install",
    "readme-main",
    "readme-run",
];

#[test]
fn published_crate_readme_matches_repository_readme() -> Result<()> {
    let repository = Dir::open_ambient_dir(repository_root(), ambient_authority())?;
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
fn installation_manifests_select_the_documented_release() -> Result<()> {
    let readme = parse_toml("readme-install")?;
    assert_dependency_version(&readme, "ortho_config", "0.9.0")?;
    assert_dependency_version(&readme, "serde", "1.0")?;

    let guide = parse_toml("guide-install")?;
    assert_dependency_version(&guide, "ortho_config", "0.9.0")?;
    assert_dependency_version(&guide, "clap", "4.5")?;
    assert_dependency_version(&guide, "serde", "1.0")?;

    let tracing = parse_toml("guide-tracing-install")?;
    let subscriber = dependency(&tracing, "tracing-subscriber")?;
    ensure!(subscriber["version"].as_str() == Some("0.3"));
    ensure!(
        subscriber["features"]
            .as_array()
            .is_some_and(|features| features
                .iter()
                .any(|value| value.as_str() == Some("env-filter"))),
        "tracing-subscriber manifest should enable env-filter"
    );
    Ok(())
}

#[test]
fn optional_and_aliased_manifests_preserve_the_intended_contract() -> Result<()> {
    let metrics = parse_toml("guide-metrics-install")?;
    let metrics_dependency = dependency(&metrics, "ortho_config")?;
    ensure!(metrics_dependency["version"].as_str() == Some("0.9.0"));
    ensure!(
        metrics_dependency["features"]
            .as_array()
            .is_some_and(|features| features
                .iter()
                .any(|value| value.as_str() == Some("metrics"))),
        "metrics manifest should enable the metrics feature"
    );

    let alias = parse_toml("guide-alias-install")?;
    let aliased_dependency = dependency(&alias, "config_layer")?;
    ensure!(aliased_dependency["package"].as_str() == Some("ortho_config"));
    ensure!(aliased_dependency["version"].as_str() == Some("0.9.0"));
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
    ensure!(value.get("enabled").and_then(serde_json_value_as_str) == Some("yes"));
    ensure!(value.get("mode").and_then(serde_json_value_as_str) == Some("on"));
    ensure!(
        value
            .get("port")
            .and_then(ortho_config::serde_json::Value::as_u64)
            == Some(8080)
    );
    Ok(())
}

#[test]
fn documented_orthohelp_command_generates_agent_context() -> Result<()> {
    let example = documented_example("guide-orthohelp-command")?;
    ensure!(example.language == "console");
    ensure!(
        example.body == "cargo orthohelp --package hello_world --format agent-context\n",
        "cargo-orthohelp command contract drifted"
    );

    let output_directory = TempDir::new().context("create orthohelp output directory")?;
    let output = Command::new("cargo")
        .args([
            "run",
            "--offline",
            "--quiet",
            "-p",
            "cargo-orthohelp",
            "--",
            "orthohelp",
            "--package",
            "hello_world",
            "--format",
            "agent-context",
            "--out-dir",
        ])
        .arg(output_directory.path())
        .current_dir(repository_root())
        .output()
        .context("run documented cargo-orthohelp flow")?;
    ensure!(
        output.status.success(),
        "cargo-orthohelp failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output_dir = Dir::open_ambient_dir(output_directory.path(), ambient_authority())?;
    let json = output_dir
        .read_to_string("agent-context.json")
        .context("read generated agent context")?;
    let payload: ortho_config::serde_json::Value = ortho_config::serde_json::from_str(&json)?;
    ensure!(payload.get("package").and_then(serde_json_value_as_str) == Some("hello_world"));
    ensure!(
        payload.get("kind").and_then(serde_json_value_as_str) == Some("hello_world.agent_context")
    );
    Ok(())
}

fn parse_toml(id: &str) -> Result<toml::Value> {
    let example = documented_example(id)?;
    ensure!(example.language == "toml", "{id} should be TOML");
    toml::from_str(&example.body).with_context(|| format!("parse {id}"))
}

fn dependency<'a>(manifest: &'a toml::Value, name: &str) -> Result<&'a toml::Value> {
    manifest
        .get("dependencies")
        .and_then(|dependencies| dependencies.get(name))
        .with_context(|| format!("manifest should declare {name}"))
}

fn assert_dependency_version(manifest: &toml::Value, name: &str, version: &str) -> Result<()> {
    let value = dependency(manifest, name)?;
    let actual = value
        .as_str()
        .or_else(|| value.get("version").and_then(toml::Value::as_str));
    ensure!(
        actual == Some(version),
        "expected {name} version {version}, got {actual:?}"
    );
    Ok(())
}

fn repository_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn serde_json_value_as_str(value: &ortho_config::serde_json::Value) -> Option<&str> {
    value.as_str()
}
