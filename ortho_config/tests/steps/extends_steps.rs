//! Steps for testing configuration inheritance.

use crate::{RulesConfig, World};
use clap::Parser;
use cucumber::{given, when};

#[given("a configuration file extending a base file")]
fn create_files(world: &mut World) {
    world.extends = true;
}

#[when("the extended configuration is loaded")]
fn load_extended(world: &mut World) {
    let mut result = None;
    figment::Jail::expect_with(|j| {
        if world.extends {
            j.create_file("base.toml", "rules = [\"base\"]")?;
            j.create_file(
                ".ddlint.toml",
                "extends = \"base.toml\"\nrules = [\"child\"]",
            )?;
        }
        let cli = RulesConfig::parse_from(["prog"]);
        result = Some(cli.load_and_merge());
        Ok(())
    });
    world.result = result;
    world.extends = false;
}
