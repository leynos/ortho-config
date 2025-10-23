//! Configuration override resolution scenarios.

use super::helpers::*;
#[cfg(unix)]
use crate::cli::discovery::collect_config_candidates;
use crate::cli::{
    GlobalArgs, GreetOverrides, apply_greet_overrides, load_config_overrides, load_global_config,
    load_greet_defaults,
};
use anyhow::{Result, anyhow, ensure};
#[cfg(unix)]
use camino::Utf8PathBuf;
use rstest::rstest;

#[rstest]
fn load_global_config_applies_overrides() -> Result<()> {
    let cli = parse_command_line(&["-r", "Team", "-s", "Hi", "greet"])?;
    let config = with_jail(|jail| {
        jail.clear_env();
        jail.set_env("HELLO_WORLD_RECIPIENT", "Team");
        jail.create_file(".hello_world.toml", "")?;
        jail.set_env("HELLO_WORLD_SALUTATIONS", "Hi");
        load_global_config(&cli.globals, None).map_err(|err| figment_error(&err))
    })?;
    ensure!(
        config.recipient == "Team",
        "unexpected recipient: {}",
        config.recipient
    );
    ensure!(
        config.trimmed_salutations() == vec![String::from("Hi")],
        "unexpected salutations"
    );
    Ok(())
}

#[rstest]
fn load_global_config_preserves_env_when_not_overridden() -> Result<()> {
    let cli = parse_command_line(&["greet"])?;
    let config = with_jail(|jail| {
        jail.clear_env();
        jail.set_env("HELLO_WORLD_RECIPIENT", "Library");
        load_global_config(&cli.globals, None).map_err(|err| figment_error(&err))
    })?;
    ensure!(
        config.recipient == "Library",
        "unexpected recipient: {}",
        config.recipient
    );
    Ok(())
}

#[rstest]
fn load_sample_configuration() -> Result<()> {
    let (config, greet_defaults) = with_jail(|jail| {
        jail.clear_env();
        let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let config_dir = cap_std::fs::Dir::open_ambient_dir(
            manifest_dir.join("config").as_std_path(),
            cap_std::ambient_authority(),
        )
        .map_err(|err| figment_error(&err))?;
        let baseline = config_dir
            .read_to_string("baseline.toml")
            .map_err(|err| figment_error(&err))?;
        let overrides = config_dir
            .read_to_string("overrides.toml")
            .map_err(|err| figment_error(&err))?;
        jail.create_file("baseline.toml", &baseline)?;
        jail.create_file(".hello_world.toml", &overrides)?;
        let config =
            load_global_config(&GlobalArgs::default(), None).map_err(|err| figment_error(&err))?;
        let greet_defaults = load_greet_defaults().map_err(|err| figment_error(&err))?;
        Ok((config, greet_defaults))
    })?;
    ensure!(config.recipient == "Excited crew", "unexpected recipient");
    ensure!(
        config.trimmed_salutations()
            == vec![String::from("Hello"), String::from("Hey config friends")],
        "unexpected salutations"
    );
    ensure!(config.is_excited, "expected excited configuration");
    assert_sample_greet_defaults(&greet_defaults)?;
    Ok(())
}

#[rstest]
fn load_config_overrides_returns_none_without_files() -> Result<()> {
    let overrides = with_jail(|jail| {
        jail.clear_env();
        load_config_overrides().map_err(|err| figment_error(&err))
    })?;
    ensure!(overrides.is_none(), "expected overrides to be absent");
    Ok(())
}

#[rstest]
fn load_config_overrides_uses_explicit_path() -> Result<()> {
    let overrides = with_jail(|jail| {
        jail.clear_env();
        jail.create_file(
            "custom.toml",
            r#"is_excited = true

[cmds.greet]
preamble = "From explicit path"
punctuation = "?"
"#,
        )?;
        jail.set_env("HELLO_WORLD_CONFIG_PATH", "custom.toml");
        load_config_overrides().map_err(|err| figment_error(&err))
    })?
    .ok_or_else(|| anyhow!("expected overrides"))?;
    ensure!(
        overrides.is_excited == Some(true),
        "unexpected excitement override"
    );
    ensure!(
        overrides.cmds.greet
            == Some(GreetOverrides {
                preamble: Some(String::from("From explicit path")),
                punctuation: Some(String::from("?")),
            }),
        "unexpected greet overrides"
    );
    Ok(())
}

#[rstest]
fn load_config_overrides_prefers_xdg_directories() -> Result<()> {
    let overrides = with_jail(|jail| {
        jail.clear_env();
        jail.create_dir("xdg")?;
        jail.create_dir("xdg/hello_world")?;
        jail.create_file(
            "xdg/hello_world/hello_world.toml",
            r#"[cmds.greet]
punctuation = "???"
"#,
        )?;
        jail.create_file(
            ".hello_world.toml",
            r#"[cmds.greet]
punctuation = "!!!"
"#,
        )?;
        jail.set_env("XDG_CONFIG_HOME", "xdg");
        load_config_overrides().map_err(|err| figment_error(&err))
    })?
    .ok_or_else(|| anyhow!("expected overrides"))?;
    ensure!(
        overrides.is_excited.is_none(),
        "unexpected excitement override"
    );
    ensure!(
        overrides.cmds.greet
            == Some(GreetOverrides {
                preamble: None,
                punctuation: Some(String::from("???")),
            }),
        "unexpected greet overrides"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn load_config_overrides_uses_xdg_fallback() -> Result<()> {
    let candidates = collect_config_candidates();
    ensure!(
        candidates.contains(&Utf8PathBuf::from("/etc/xdg/hello_world/hello_world.toml")),
        "expected fallback hello world config in candidate list"
    );
    ensure!(
        candidates.contains(&Utf8PathBuf::from("/etc/xdg/.hello_world.toml")),
        "expected fallback dotfile config in candidate list"
    );
    Ok(())
}

#[rstest]
fn load_config_overrides_reads_localappdata() -> Result<()> {
    let overrides = with_jail(|jail| {
        jail.clear_env();
        jail.create_dir("localdata")?;
        jail.create_dir("localdata/hello_world")?;
        jail.create_file(
            "localdata/hello_world/hello_world.toml",
            "is_excited = true",
        )?;
        jail.create_file(".hello_world.toml", "is_excited = false")?;
        jail.set_env("LOCALAPPDATA", "localdata");
        load_config_overrides().map_err(|err| figment_error(&err))
    })?
    .ok_or_else(|| anyhow!("expected overrides"))?;
    ensure!(
        overrides.is_excited == Some(true),
        "unexpected excitement override"
    );
    Ok(())
}

#[rstest]
fn apply_greet_overrides_updates_command(greet_command: GreetCommandFixture) -> Result<()> {
    let mut command = greet_command?;
    with_jail(|jail| {
        jail.clear_env();
        jail.create_file(
            ".hello_world.toml",
            r#"[cmds.greet]
preamble = "From file"
punctuation = "?!"
"#,
        )?;
        apply_greet_overrides(&mut command).map_err(|err| figment_error(&err))
    })?;
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
