//! Configuration override resolution scenarios.

use super::helpers::*;
#[cfg(unix)]
use crate::cli::discovery::collect_config_candidates;
use crate::cli::{
    CommandOverrides, FileOverrides, GlobalArgs, GreetOverrides, apply_greet_overrides_with_source,
    load_config_overrides_from_discovery, load_global_config_with_sources,
    load_greet_defaults_with_sources,
};
use anyhow::{Context, Result, anyhow, ensure};
use camino::Utf8PathBuf;
use ortho_config::{MapEnv, SharedEnvSource};
use rstest::rstest;
use std::sync::Arc;

const PROGRAM_NAME: &str = "hello-world";

#[rstest]
fn load_global_config_preserves_env_when_not_overridden() -> Result<()> {
    let cli = parse_command_line(&["greet"])?;
    let fixture = ConfigFixture::new()?;
    let selected = fixture.write("empty.toml", "")?;
    let discovery = MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", selected);
    let merge = MapEnv::new().with_var("HELLO_WORLD_RECIPIENT", "Library");
    let config = load_global_config_with_sources(
        &cli.globals,
        None,
        PROGRAM_NAME,
        global_sources(discovery, merge),
    )?;
    ensure!(
        config.recipient == "Library",
        "unexpected recipient: {}",
        config.recipient
    );
    Ok(())
}

#[rstest]
fn load_sample_configuration() -> Result<()> {
    let fixture = ConfigFixture::new()?;
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_dir = cap_std::fs::Dir::open_ambient_dir(
        manifest_dir.join("config").as_std_path(),
        cap_std::ambient_authority(),
    )
    .context("open sample configuration")?;
    let baseline = config_dir.read_to_string("baseline.toml")?;
    let overrides = config_dir.read_to_string("overrides.toml")?;
    fixture.write("baseline.toml", &baseline)?;
    let selected = fixture.write(".hello_world.toml", &overrides)?;
    let discovery = MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", selected);
    let config = load_global_config_with_sources(
        &GlobalArgs::default(),
        None,
        PROGRAM_NAME,
        global_sources(discovery.clone(), MapEnv::new()),
    )?;
    let greet_defaults =
        load_greet_defaults_with_sources(fixture.path(), global_sources(discovery, MapEnv::new()))?;
    ensure!(config.recipient == "Excited crew", "unexpected recipient");
    // With declarative merge semantics, Vec<T> appends across defaults + extends chain
    ensure!(
        config.trimmed_salutations()
            == vec![
                "Hello".to_owned(),
                "Hello from config".to_owned(),
                "Hey config friends".to_owned(),
            ],
        "unexpected salutations"
    );
    ensure!(config.is_excited, "expected excited configuration");
    assert_sample_greet_defaults(&greet_defaults)?;
    Ok(())
}

#[rstest]
fn load_config_overrides_returns_none_without_files() -> Result<()> {
    let fixture = ConfigFixture::new()?;
    let source = MapEnv::new().with_var("XDG_CONFIG_DIRS", fixture.path());
    let discovery = fixture.discovery(source);
    let overrides = load_config_overrides_from_discovery(&discovery)?;
    ensure!(overrides.is_none(), "expected overrides to be absent");
    Ok(())
}

#[rstest]
fn load_config_overrides_returns_path() -> Result<()> {
    let fixture = ConfigFixture::new()?;
    let selected = fixture.write(".hello_world.toml", "is_excited = true")?;
    let discovery = fixture.discovery(MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", selected));
    let (overrides, path) = load_config_overrides_from_discovery(&discovery)?
        .ok_or_else(|| anyhow!("expected overrides to load"))?;
    ensure!(
        overrides.is_excited == Some(true),
        "unexpected overrides value"
    );
    let file_name = path
        .as_deref()
        .and_then(|p| p.file_name())
        .map(std::string::ToString::to_string);
    ensure!(
        file_name.as_deref() == Some(".hello_world.toml"),
        "expected returned path to match discovered file"
    );
    Ok(())
}

#[rstest]
fn load_global_config_prefers_cli_excited_flag() -> Result<()> {
    let cli = parse_command_line(&["--is-excited", "greet"])?;
    let fixture = ConfigFixture::new()?;
    let selected = fixture.write(".hello_world.toml", "is_excited = false")?;
    let discovery = MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", selected);
    let config = load_global_config_with_sources(
        &cli.globals,
        None,
        PROGRAM_NAME,
        global_sources(discovery, MapEnv::new()),
    )?;
    ensure!(
        config.is_excited,
        "cli excited flag should override file value"
    );
    Ok(())
}

#[cfg(feature = "yaml")]
#[rstest]
fn load_yaml_config_activates_excited_flag() -> Result<()> {
    let cli = parse_command_line(&["--config", "canonical.yaml", "greet"])?;
    let fixture = ConfigFixture::new()?;
    let selected = fixture.write("canonical.yaml", "is_excited: true")?;
    let fig = ortho_config::load_config_file(&selected)?
        .ok_or_else(|| anyhow!("missing canonical.yaml"))?;
    let is_excited: bool = fig.extract_inner("is_excited")?;
    ensure!(is_excited, "expected canonical bool to parse as true");
    let config = load_global_config_with_sources(
        &cli.globals,
        Some(&selected),
        PROGRAM_NAME,
        global_sources(MapEnv::new(), MapEnv::new()),
    )?;
    ensure!(config.is_excited, "expected excited configuration");
    Ok(())
}

#[rstest]
fn load_global_config_uses_explicit_override_file() -> Result<()> {
    let cli = parse_command_line(&["greet"])?;
    let fixture = ConfigFixture::new()?;
    let discovered = fixture.write(".hello_world.toml", "is_excited = false")?;
    let override_path = fixture.write("override.toml", "is_excited = true")?;
    let discovery = MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", discovered);
    let config = load_global_config_with_sources(
        &cli.globals,
        Some(&override_path),
        PROGRAM_NAME,
        global_sources(discovery, MapEnv::new()),
    )?;
    ensure!(
        config.is_excited,
        "explicit override path should take precedence"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn load_config_overrides_uses_xdg_fallback() -> Result<()> {
    let candidates = collect_config_candidates(Arc::new(MapEnv::new()));
    ensure!(
        candidates.contains(&Utf8PathBuf::from("/etc/xdg/hello_world/hello_world.toml")),
        "expected fallback hello world config in candidate list"
    );
    ensure!(
        candidates.contains(&Utf8PathBuf::from("/etc/xdg/.hello_world.toml")),
        "expected fallback dotfile config in candidate list"
    );

    let fixture = ConfigFixture::new()?;
    let override_dir = fixture.path().join("xdg-override");
    let override_candidates = collect_config_candidates(Arc::new(
        MapEnv::new().with_var("XDG_CONFIG_DIRS", &override_dir),
    ));
    ensure!(
        override_candidates
            .iter()
            .any(|candidate| candidate.as_std_path()
                == override_dir.join("hello_world/hello_world.toml")),
        "expected injected XDG directory in candidate list"
    );
    ensure!(
        !override_candidates.contains(&Utf8PathBuf::from("/etc/xdg/hello_world/hello_world.toml")),
        "injected XDG directory must replace the platform fallback"
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum OverrideSource {
    Explicit,
    Xdg,
    LocalAppData,
}

fn prepare_override_source(fixture: &ConfigFixture, source: OverrideSource) -> Result<MapEnv> {
    match source {
        OverrideSource::Explicit => prepare_explicit_override_source(fixture),
        OverrideSource::Xdg => prepare_xdg_override_source(fixture),
        OverrideSource::LocalAppData => prepare_local_app_data_override_source(fixture),
    }
}

fn prepare_explicit_override_source(fixture: &ConfigFixture) -> Result<MapEnv> {
    let selected = fixture.write(
        "custom.toml",
        r#"is_excited = true

[cmds.greet]
preamble = "From explicit path"
punctuation = "?"
"#,
    )?;
    Ok(MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", selected))
}

fn prepare_xdg_override_source(fixture: &ConfigFixture) -> Result<MapEnv> {
    fixture.create_dir("xdg/hello_world")?;
    fixture.write(
        "xdg/hello_world/hello_world.toml",
        "[cmds.greet]\npunctuation = \"???\"\n",
    )?;
    fixture.write(".hello_world.toml", "[cmds.greet]\npunctuation = \"!!!\"\n")?;
    Ok(MapEnv::new().with_var("XDG_CONFIG_HOME", fixture.path().join("xdg")))
}

fn prepare_local_app_data_override_source(fixture: &ConfigFixture) -> Result<MapEnv> {
    fixture.create_dir("localdata/hello_world")?;
    fixture.write(
        "localdata/hello_world/hello_world.toml",
        "is_excited = true",
    )?;
    fixture.write(".hello_world.toml", "is_excited = false")?;
    Ok(MapEnv::new().with_var("LOCALAPPDATA", fixture.path().join("localdata")))
}

#[rstest]
#[case::explicit(
    OverrideSource::Explicit,
    FileOverrides {
        is_excited: Some(true),
        cmds: CommandOverrides {
            greet: Some(GreetOverrides {
                preamble: Some("From explicit path".to_owned()),
                punctuation: Some("?".to_owned()),
            }),
        },
    }
)]
#[case::xdg(
    OverrideSource::Xdg,
    FileOverrides {
        is_excited: None,
        cmds: CommandOverrides {
            greet: Some(GreetOverrides {
                preamble: None,
                punctuation: Some("???".to_owned()),
            }),
        },
    }
)]
#[case::localappdata(
    OverrideSource::LocalAppData,
    FileOverrides {
        is_excited: Some(true),
        cmds: CommandOverrides { greet: None },
    }
)]
fn load_config_overrides_sources(
    #[case] source: OverrideSource,
    #[case] expected: FileOverrides,
) -> Result<()> {
    let fixture = ConfigFixture::new()?;
    let env = prepare_override_source(&fixture, source)?;
    let discovery = fixture.discovery(env);
    let (overrides, _) = load_config_overrides_from_discovery(&discovery)?
        .ok_or_else(|| anyhow!("expected overrides"))?;
    ensure!(
        overrides == expected,
        "unexpected overrides: {overrides:?}; expected {expected:?}"
    );
    Ok(())
}

#[rstest]
fn apply_greet_overrides_updates_command(greet_command: GreetCommandFixture) -> Result<()> {
    let mut command = greet_command?;
    let fixture = ConfigFixture::new()?;
    let selected = fixture.write(
        ".hello_world.toml",
        r#"[cmds.greet]
preamble = "From file"
punctuation = "?!"
"#,
    )?;
    let discovery: SharedEnvSource =
        Arc::new(MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", selected));
    apply_greet_overrides_with_source(&mut command, discovery)?;
    ensure!(
        command.preamble.as_deref() == Some("From file"),
        "unexpected preamble override"
    );
    ensure!(
        command.punctuation == "?!",
        "unexpected punctuation override"
    );
    Ok(())
}
