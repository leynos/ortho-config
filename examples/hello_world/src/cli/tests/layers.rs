//! Regression tests for the declarative global loader.
//!
//! These cases exercise the compose_layers-based implementation that replaces
//! the bespoke `load_global_config` helper. The assertions mirror the merge
//! order promised in the design doc: defaults, discovered files, environment,
//! then CLI.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::helpers::parse_command_line;
use crate::cli::{
    GlobalArgs, GlobalConfigSources, HelloWorldCli, load_config_overrides_from_discovery,
    load_global_config_with_sources, load_greet_defaults_with_sources,
};
use anyhow::{Context as _, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{ConfigDiscovery, MapEnv, SharedEnvSource, SharedScanEnvSource};
use rstest::rstest;
use tempfile::TempDir;

/// A capability-scoped directory containing the global loader's test fixtures.
///
/// This stays local to layer tests: the five cases need short, named fixtures,
/// while other CLI suites exercise their own discovery arrangements.
struct LayerFixture {
    root: TempDir,
}

impl LayerFixture {
    fn new() -> Result<Self> {
        Ok(Self {
            root: tempfile::tempdir().context("create global configuration fixture")?,
        })
    }

    fn write(&self, name: &str, contents: &str) -> Result<PathBuf> {
        let directory = Dir::open_ambient_dir(self.root.path(), ambient_authority())
            .context("open global configuration fixture directory")?;
        directory
            .write(name, contents.as_bytes())
            .context("write global configuration fixture")?;
        Ok(self.root.path().join(name))
    }
}

/// Resolve one global configuration with separate discovery and merge sources.
fn load_with_sources(
    globals: &GlobalArgs,
    config_override: Option<&Path>,
    discovery: MapEnv,
    merge: MapEnv,
) -> Result<HelloWorldCli> {
    let discovery_source: SharedEnvSource = Arc::new(discovery);
    let merge_source: SharedScanEnvSource = Arc::new(merge);
    load_global_config_with_sources(
        globals,
        config_override,
        "hello-world",
        GlobalConfigSources::new(discovery_source, merge_source),
    )
    .context("load global configuration from injected sources")
}

/// Assert the injected selector is ahead of every fallback candidate.
fn assert_selector_is_first(selected: &Path, discovery: &MapEnv) -> Result<()> {
    let candidates = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .clear_project_roots()
        .env_source(Arc::new(discovery.clone()))
        .build()
        .candidates();
    ensure!(
        candidates.first() == Some(&selected.to_path_buf()),
        "injected selector must be the first candidate: {candidates:?}"
    );
    Ok(())
}

fn environment_from_assignment(assignment: Option<&str>) -> Result<MapEnv> {
    match assignment {
        Some(raw_assignment) => {
            let (key, value) = raw_assignment
                .split_once('=')
                .ok_or_else(|| anyhow!("expected environment key=value assignment"))?;
            Ok(MapEnv::new().with_var(key, value))
        }
        None => Ok(MapEnv::new()),
    }
}

#[test]
fn injected_discovery_has_no_file_in_an_explicit_empty_root() -> Result<()> {
    let fixture = LayerFixture::new()?;
    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .project_roots([fixture.root.path().to_path_buf()])
        .env_source(Arc::new(
            MapEnv::new().with_var("XDG_CONFIG_DIRS", fixture.root.path()),
        ))
        .build();

    let overrides = load_config_overrides_from_discovery(&discovery)?;
    ensure!(
        overrides.is_none(),
        "empty injected root should not yield an override"
    );
    Ok(())
}

#[test]
fn injected_greeting_defaults_use_file_and_merge_sources() -> Result<()> {
    let fixture = LayerFixture::new()?;
    fixture.write(".hello_world.toml", "[cmds.greet]\npunctuation = \"??\"\n")?;
    let selected = fixture.write("greet.toml", "[cmds.greet]\npreamble = \"From file\"\n")?;
    let discovery: SharedEnvSource = Arc::new(
        MapEnv::new()
            .with_var("HELLO_WORLD_CONFIG_PATH", &selected)
            .with_var("XDG_CONFIG_DIRS", fixture.root.path()),
    );
    let merge: SharedScanEnvSource =
        Arc::new(MapEnv::new().with_var("HELLO_WORLD_CMDS_GREET_PREAMBLE", "From merge"));

    let command = load_greet_defaults_with_sources(
        fixture.root.path(),
        GlobalConfigSources::new(discovery, merge),
    )?;
    ensure!(
        command.preamble.as_deref() == Some("From file"),
        "file greeting override should apply after the merge layer"
    );
    ensure!(
        command.punctuation == "??",
        "punctuation from the explicit file base: expected ??, got {:?}",
        command.punctuation
    );
    Ok(())
}

#[rstest]
#[case::file_env_cli(
    &["-s", "CliSalutation", "greet"],
    r#"salutations = ["File"]"#,
    Some("HELLO_WORLD_SALUTATIONS=EnvOne,EnvTwo"),
    vec!["CliSalutation"],
)]
#[case::env_only(
    &["greet"],
    r#"salutations = ["File"]"#,
    Some("HELLO_WORLD_SALUTATIONS=EnvOnly"),
    vec!["Hello", "File", "EnvOnly"],
)]
#[case::file_only(
    &["greet"],
    r#"salutations = ["FileOnly"]"#,
    None,
    vec!["Hello", "FileOnly"],
)]
fn load_global_config_accumulates_salutations(
    #[case] cli_args: &[&str],
    #[case] file_contents: &str,
    #[case] env_var: Option<&str>,
    #[case] expected: Vec<&str>,
) -> Result<()> {
    let cli = parse_command_line(cli_args)?;
    let fixture = LayerFixture::new()?;
    let selected = fixture.write(".hello_world.toml", file_contents)?;
    let discovery = MapEnv::new()
        .with_var("HELLO_WORLD_CONFIG_PATH", &selected)
        .with_var("HELLO_WORLD_SALUTATIONS", "DiscoveryOnly");
    assert_selector_is_first(&selected, &discovery)?;
    let merge = environment_from_assignment(env_var)?;
    let merged = load_with_sources(&cli.globals, None, discovery, merge)?;

    ensure!(
        merged.salutations == expected,
        "expected layered salutations {expected:?}, got {:?}",
        merged.salutations
    );
    Ok(())
}

#[rstest]
fn load_global_config_trims_cli_salutations() -> Result<()> {
    let cli = parse_command_line(&["-s", "  Hello  ", "greet"])?;
    let fixture = LayerFixture::new()?;
    let selected = fixture.write("trim.toml", "")?;
    let discovery = MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", &selected);
    let merged = load_with_sources(&cli.globals, None, discovery, MapEnv::new())?;
    ensure!(
        merged.salutations.ends_with(&[String::from("Hello")]),
        "CLI salutation should be trimmed"
    );
    Ok(())
}

#[rstest]
fn load_global_config_respects_explicit_override() -> Result<()> {
    let cli = GlobalArgs::default();
    let fixture = LayerFixture::new()?;
    let discovered = fixture.write("discovered.toml", r#"recipient = "Discovered""#)?;
    let override_path = fixture.write("override.toml", r#"recipient = "Explicit""#)?;
    let discovery = MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", &discovered);
    let merged = load_with_sources(&cli, Some(&override_path), discovery, MapEnv::new())?;
    ensure!(
        merged.recipient == "Explicit",
        "explicit override path should take precedence"
    );
    Ok(())
}
