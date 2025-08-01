use crate::{PrArgs, World};
use clap::Parser;
use cucumber::{given, then, when};
use ortho_config::subcommand::load_and_merge_subcommand_for;

/// Check if all configuration sources are absent.
fn has_no_config_sources(world: &World) -> bool {
    world.sub_ref.is_none() && world.sub_file.is_none() && world.sub_env.is_none()
}

#[given(expr = "a CLI reference {string}")]
fn set_cli_ref(world: &mut World, val: String) {
    world.sub_ref = Some(val);
}

#[given("no CLI reference")]
fn no_cli_ref(world: &mut World) {
    world.sub_ref = None;
}

#[given(expr = "a configuration reference {string}")]
fn file_ref(world: &mut World, val: String) {
    world.sub_file = Some(val);
}

#[given(expr = "an environment reference {string}")]
fn env_ref(world: &mut World, val: String) {
    world.sub_env = Some(val);
}

#[when("the subcommand configuration is loaded without defaults")]
fn load_sub(world: &mut World) {
    let mut result = None;
    if has_no_config_sources(world) {
        result = Some(PrArgs::try_parse_from(["test"]).map_err(Into::into));
    } else {
        let cli = PrArgs {
            reference: world.sub_ref.clone(),
        };
        figment::Jail::expect_with(|j| {
            if let Some(ref val) = world.sub_file {
                j.create_file(".app.toml", &format!("[cmds.test]\nreference = \"{val}\""))?;
            }
            if let Some(ref val) = world.sub_env {
                j.set_env("APP_CMDS_TEST_REFERENCE", val);
            }
            result = Some(load_and_merge_subcommand_for::<PrArgs>(&cli));
            Ok(())
        });
    }
    world.sub_result = result;
    world.sub_file = None;
    world.sub_env = None;
}

#[then(expr = "the merged reference is {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step requires owned String"
)]
fn check_ref(world: &mut World, expected: String) {
    let cfg = world.sub_result.take().expect("result").expect("ok");
    assert_eq!(cfg.reference.as_deref(), Some(expected.as_str()));
}

#[then("the subcommand load fails")]
fn sub_error(world: &mut World) {
    assert!(world.sub_result.take().expect("result").is_err());
}
