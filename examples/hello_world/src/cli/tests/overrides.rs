//! Configuration override resolution scenarios.

use super::helpers::*;
#[cfg(unix)]
use crate::cli::discovery::collect_config_candidates;
use crate::cli::{
    CommandOverrides, FileOverrides, GlobalArgs, GreetOverrides, apply_greet_overrides,
    load_config_overrides, load_global_config, load_greet_defaults,
};
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use ortho_config::figment;
use rstest::rstest;
use std::path::Path;

const PROGRAM_NAME: &str = "hello-world";

#[rstest]
fn load_global_config_preserves_env_when_not_overridden() -> Result<()> {
    let cli = parse_command_line(&["greet"])?;
    let config = with_jail(|jail| {
        jail.clear_env();
        jail.set_env("HELLO_WORLD_RECIPIENT", "Library");
        load_global_config(&cli.globals, None, PROGRAM_NAME).map_err(figment_error)
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
        .map_err(figment_error)?;
        let baseline = config_dir
            .read_to_string("baseline.toml")
            .map_err(figment_error)?;
        let overrides = config_dir
            .read_to_string("overrides.toml")
            .map_err(figment_error)?;
        jail.create_file("baseline.toml", &baseline)?;
        jail.create_file(".hello_world.toml", &overrides)?;
        let config = load_global_config(&GlobalArgs::default(), None, PROGRAM_NAME)
            .map_err(figment_error)?;
        let greet_defaults = load_greet_defaults().map_err(figment_error)?;
        Ok((config, greet_defaults))
    })?;
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
    let overrides = load_overrides_in_jail(|jail| {
        jail.clear_env();
        Ok(())
    })?;
    ensure!(overrides.is_none(), "expected overrides to be absent");
    Ok(())
}

#[rstest]
fn load_config_overrides_returns_path() -> Result<()> {
    let (overrides, path) = with_jail(|jail| {
        jail.clear_env();
        jail.create_file(".hello_world.toml", "is_excited = true")?;
        let result = load_config_overrides().map_err(figment_error)?;
        result.ok_or_else(|| figment::Error::from("expected overrides to load"))
    })?;
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
    let config = with_jail(|jail| {
        jail.clear_env();
        jail.create_file(".hello_world.toml", "is_excited = false")?;
        load_global_config(&cli.globals, None, PROGRAM_NAME).map_err(figment_error)
    })?;
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
    let config = with_jail(|jail| {
        jail.clear_env();
        jail.create_file("canonical.yaml", "is_excited: true")?;
        if let Some(fig) = ortho_config::load_config_file(std::path::Path::new("canonical.yaml"))
            .map_err(figment_error)?
        {
            let is_excited: bool = fig.extract_inner("is_excited").map_err(figment_error)?;
            if !is_excited {
                return Err(figment::Error::from(
                    "expected canonical bool to parse as true",
                ));
            }
        } else {
            return Err(figment::Error::from("missing canonical.yaml"));
        }
        load_global_config(&cli.globals, cli.config_path.as_deref(), PROGRAM_NAME)
            .map_err(figment_error)
    })?;
    ensure!(config.is_excited, "expected excited configuration");
    Ok(())
}

#[rstest]
fn load_global_config_uses_explicit_override_file() -> Result<()> {
    let cli = parse_command_line(&["greet"])?;
    let config = with_jail(|jail| {
        jail.clear_env();
        jail.create_file(".hello_world.toml", "is_excited = false")?;
        jail.create_file("override.toml", "is_excited = true")?;
        load_global_config(&cli.globals, Some(Path::new("override.toml")), PROGRAM_NAME)
            .map_err(figment_error)
    })?;
    ensure!(
        config.is_excited,
        "explicit override path should take precedence"
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
#[case::explicit(
    |j: &mut figment::Jail| {
        j.clear_env();
        j.create_file(
            "custom.toml",
            r#"is_excited = true

[cmds.greet]
preamble = "From explicit path"
punctuation = "?"
"#,
        )?;
        j.set_env("HELLO_WORLD_CONFIG_PATH", "custom.toml");
        Ok(())
    },
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
    |j: &mut figment::Jail| {
        j.clear_env();
        j.create_dir("xdg")?;
        j.create_dir("xdg/hello_world")?;
        j.create_file(
            "xdg/hello_world/hello_world.toml",
            r#"[cmds.greet]
punctuation = "???"
"#,
        )?;
        j.create_file(
            ".hello_world.toml",
            r#"[cmds.greet]
punctuation = "!!!"
"#,
        )?;
        j.set_env("XDG_CONFIG_HOME", "xdg");
        Ok(())
    },
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
    |j: &mut figment::Jail| {
        j.clear_env();
        j.create_dir("localdata")?;
        j.create_dir("localdata/hello_world")?;
        j.create_file(
            "localdata/hello_world/hello_world.toml",
            "is_excited = true",
        )?;
        j.create_file(".hello_world.toml", "is_excited = false")?;
        j.set_env("LOCALAPPDATA", "localdata");
        Ok(())
    },
    FileOverrides {
        is_excited: Some(true),
        cmds: CommandOverrides { greet: None },
    }
)]
fn load_config_overrides_sources(
    #[case] setup: fn(&mut figment::Jail) -> figment::error::Result<()>,
    #[case] expected: FileOverrides,
) -> Result<()> {
    let overrides = expect_overrides(setup)?;
    ensure!(
        overrides == expected,
        "unexpected overrides: {overrides:?}; expected {expected:?}"
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
        apply_greet_overrides(&mut command).map_err(figment_error)
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
