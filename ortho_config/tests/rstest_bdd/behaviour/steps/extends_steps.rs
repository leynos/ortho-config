//! Steps for testing configuration inheritance.

use crate::fixtures::{ExtendsContext, RulesConfig};
use anyhow::{Result, anyhow, ensure};
use ortho_config::{OrthoConfig, OrthoResult};
use rstest_bdd::Slot;
use rstest_bdd_macros::{given, then, when};
use test_helpers::figment as figment_helpers;

#[given("a configuration file extending a base file")]
fn create_files(extends_context: &ExtendsContext) -> Result<()> {
    ensure!(
        extends_context.extends_flag.is_empty(),
        "extended configuration already initialised"
    );
    extends_context.extends_flag.set(());
    Ok(())
}

#[given("a configuration file with cyclic inheritance")]
fn create_cyclic(extends_context: &ExtendsContext) -> Result<()> {
    ensure!(
        extends_context.cyclic_flag.is_empty(),
        "cyclic configuration already initialised"
    );
    extends_context.cyclic_flag.set(());
    Ok(())
}

#[given("a configuration file extending a missing base file")]
fn create_missing_base(extends_context: &ExtendsContext) -> Result<()> {
    ensure!(
        extends_context.missing_base_flag.is_empty(),
        "missing-base configuration already initialised"
    );
    extends_context.missing_base_flag.set(());
    Ok(())
}

#[given("a configuration file extending a parent file that extends a grandparent file")]
fn create_multi_level(extends_context: &ExtendsContext) -> Result<()> {
    ensure!(
        extends_context.multi_level_flag.is_empty(),
        "multi-level configuration already initialised"
    );
    extends_context.multi_level_flag.set(());
    Ok(())
}

fn with_jail_load<F>(setup: F) -> Result<OrthoResult<RulesConfig>>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
{
    figment_helpers::with_jail(|j| {
        setup(j)?;
        Ok(RulesConfig::load_from_iter(["prog"]))
    })
}

fn load_with_flag<F>(
    flag: &Slot<()>,
    flag_name: &str,
    setup: F,
    extends_context: &ExtendsContext,
) -> Result<()>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
{
    ensure!(flag.is_filled(), "{flag_name} was not initialised");
    flag.clear();
    let result = with_jail_load(setup)?;
    extends_context.result.set(result);
    Ok(())
}

#[derive(Copy, Clone)]
enum ExtendsScenario {
    Extended,
    Cyclic,
    MissingBase,
    MultiLevel,
}

impl ExtendsScenario {
    fn flag<'a>(&self, context: &'a ExtendsContext) -> &'a Slot<()> {
        match self {
            Self::Extended => &context.extends_flag,
            Self::Cyclic => &context.cyclic_flag,
            Self::MissingBase => &context.missing_base_flag,
            Self::MultiLevel => &context.multi_level_flag,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Extended => "extended configuration",
            Self::Cyclic => "cyclic configuration",
            Self::MissingBase => "missing-base configuration",
            Self::MultiLevel => "multi-level configuration",
        }
    }

    fn setup(self, j: &mut figment::Jail) -> figment::error::Result<()> {
        match self {
            Self::Extended => {
                j.create_file("base.toml", "rules = [\"base\"]")?;
                j.create_file(
                    ".ddlint.toml",
                    "extends = \"base.toml\"\nrules = [\"child\"]",
                )?;
            }
            Self::Cyclic => {
                j.create_file("a.toml", "extends = \"b.toml\"\nrules = [\"a\"]")?;
                j.create_file("b.toml", "extends = \"a.toml\"\nrules = [\"b\"]")?;
                j.create_file(".ddlint.toml", "extends = \"a.toml\"")?;
            }
            Self::MissingBase => {
                j.create_file(
                    ".ddlint.toml",
                    "extends = \"missing.toml\"\nrules = [\"main\"]",
                )?;
            }
            Self::MultiLevel => {
                let grandparent = concat!("rules = [\"grandparent\"]\n");
                let parent = concat!(
                    "extends = \"grandparent.toml\"\n",
                    "rules = [\"parent\"]\n",
                );
                let child = concat!(
                    "extends = \"parent.toml\"\n",
                    "rules = [\"child\"]\n",
                );
                j.create_file("grandparent.toml", grandparent)?;
                j.create_file("parent.toml", parent)?;
                j.create_file(".ddlint.toml", child)?;
            }
        }
        Ok(())
    }
}

fn load_scenario(scenario: ExtendsScenario, context: &ExtendsContext) -> Result<()> {
    load_with_flag(
        scenario.flag(context),
        scenario.name(),
        |j| scenario.setup(j),
        context,
    )
}

#[when("the extended configuration is loaded")]
fn load_extended(extends_context: &ExtendsContext) -> Result<()> {
    load_scenario(ExtendsScenario::Extended, extends_context)
}

#[when("the cyclic configuration is loaded")]
fn load_cyclic(extends_context: &ExtendsContext) -> Result<()> {
    load_scenario(ExtendsScenario::Cyclic, extends_context)
}

#[when("the configuration with missing base is loaded")]
fn load_missing_base(extends_context: &ExtendsContext) -> Result<()> {
    load_scenario(ExtendsScenario::MissingBase, extends_context)
}

#[when("the multi-level configuration is loaded")]
fn load_multi_level(extends_context: &ExtendsContext) -> Result<()> {
    load_scenario(ExtendsScenario::MultiLevel, extends_context)
}

#[then("an error occurs")]
fn error_occurs(extends_context: &ExtendsContext) -> Result<()> {
    let result = extends_context
        .result
        .take()
        .ok_or_else(|| anyhow!("configuration result unavailable"))?;
    ensure!(result.is_err(), "expected configuration to fail");
    Ok(())
}

fn strip_rule_quotes(value: &str) -> &str {
    let trimmed = value.trim();
    if let Some(stripped) = trimmed.strip_prefix('"').and_then(|val| val.strip_suffix('"')) {
        return stripped;
    }
    if let Some(stripped) = trimmed.strip_prefix('\'').and_then(|val| val.strip_suffix('\'')) {
        return stripped;
    }
    trimmed
}

fn parse_rules_list(rules: &str) -> Vec<String> {
    rules
        .split(',')
        .map(|value| strip_rule_quotes(value))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

#[then("the inherited rules are {rules}")]
fn inherited_rules(extends_context: &ExtendsContext, rules: String) -> Result<()> {
    let actual = extends_context
        .result
        .with_ref(|result| result.as_ref().map(|cfg| cfg.rules.clone()))
        .ok_or_else(|| anyhow!("configuration result unavailable"))?
        .map_err(|err| anyhow!(err))?;
    let expected = parse_rules_list(&rules);
    ensure!(
        actual == expected,
        "unexpected rules {:?}; expected {:?}",
        actual,
        expected
    );
    Ok(())
}

#[then("the rules are {rules}")]
fn rules_are(extends_context: &ExtendsContext, rules: String) -> Result<()> {
    inherited_rules(extends_context, rules)
}
