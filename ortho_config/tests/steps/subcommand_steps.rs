use crate::{PrArgs, World};
use cucumber::{given, then, when};
use ortho_config::subcommand::load_and_merge_subcommand_for;

#[given(expr = "a CLI reference {string}")]
fn set_cli_ref(world: &mut World, val: String) {
    world.sub_ref = Some(val);
}

#[when("the subcommand configuration is loaded without defaults")]
fn load_sub(world: &mut World) {
    let val = world.sub_ref.clone().expect("ref");
    let cli = PrArgs {
        reference: Some(val),
    };
    let mut result = None;
    figment::Jail::expect_with(|_| {
        result = Some(load_and_merge_subcommand_for::<PrArgs>(&cli));
        Ok(())
    });
    world.sub_result = result;
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
