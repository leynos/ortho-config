//! Steps for testing configuration inheritance.

use crate::{RulesConfig, World};
use clap::Parser;
use cucumber::{given, then, when};

#[given("a configuration file extending a base file")]
fn create_files(world: &mut World) {
    world.extends = true;
}

#[given("a configuration file with cyclic inheritance")]
fn create_cyclic(world: &mut World) {
    world.cyclic = true;
}

#[given("a configuration file extending a missing base file")]
fn create_missing_base(world: &mut World) {
    world.missing_base = true;
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

#[when("the cyclic configuration is loaded")]
fn load_cyclic(world: &mut World) {
    let mut result = None;
    figment::Jail::expect_with(|j| {
        j.create_file("a.toml", "extends = \"b.toml\"\nrules = [\"a\"]")?;
        j.create_file("b.toml", "extends = \"a.toml\"\nrules = [\"b\"]")?;
        j.create_file(".ddlint.toml", "extends = \"a.toml\"")?;
        let cli = RulesConfig::parse_from(["prog"]);
        result = Some(cli.load_and_merge());
        Ok(())
    });
    world.result = result;
    world.cyclic = false;
}

#[when("the configuration with missing base is loaded")]
fn load_missing_base(world: &mut World) {
    let mut result = None;
    figment::Jail::expect_with(|j| {
        j.create_file(
            ".ddlint.toml",
            "extends = \"missing.toml\"\nrules = [\"main\"]",
        )?;
        let cli = RulesConfig::parse_from(["prog"]);
        result = Some(cli.load_and_merge());
        Ok(())
    });
    world.result = result;
    world.missing_base = false;
}

#[then("an error occurs")]
fn error_occurs(world: &mut World) {
    assert!(world.result.take().expect("result").is_err());
}
