//! Steps for testing configuration inheritance.

use super::common::SlotTakeOrExt;
use crate::scenario_state::{ExtendsContext, ReplaceRulesConfig, RulesConfig};
use anyhow::{Context as _, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, OrthoConfig, OrthoResult, SharedEnvSource, SharedScanEnvSource};
use rstest_bdd::Slot;
use rstest_bdd_macros::{given, then, when};
use std::{ffi::OsString, path::PathBuf, sync::Arc};

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

#[given("a configuration file with a non-string extends value")]
fn create_non_string(extends_context: &ExtendsContext) -> Result<()> {
    ensure!(
        extends_context.non_string_flag.is_empty(),
        "non-string configuration already initialized"
    );
    extends_context.non_string_flag.set(());
    Ok(())
}

#[given("a configuration file extending a base file with replace strategy on rules")]
fn create_replace_strategy(extends_context: &ExtendsContext) -> Result<()> {
    ensure!(
        extends_context.replace_strategy_flag.is_empty(),
        "replace-strategy configuration already initialised"
    );
    extends_context.replace_strategy_flag.set(());
    Ok(())
}

/// Write one fixture file into this module's isolated extends graph.
fn write_fixture(dir: &Dir, relative_path: &str, contents: &str) -> Result<()> {
    dir.write(relative_path, contents.as_bytes())
        .with_context(|| format!("write extends fixture {relative_path}"))?;
    Ok(())
}

/// Prepare an isolated extends graph and its closed discovery and merge sources.
///
/// This is private to the BDD extends scenarios. Callers must keep the returned
/// `TempDir` alive while the generated loader reads the required child path.
fn prepare_fixture<F>(
    setup: F,
) -> Result<(
    tempfile::TempDir,
    PathBuf,
    SharedEnvSource,
    SharedScanEnvSource,
)>
where
    F: FnOnce(&Dir) -> Result<()>,
{
    let fixture_dir = tempfile::tempdir().context("create extends fixture directory")?;
    let dir = Dir::open_ambient_dir(fixture_dir.path(), ambient_authority())
        .context("open extends fixture directory")?;
    setup(&dir)?;
    let child_path = fixture_dir.path().join(".ddlint.toml");
    let source = Arc::new(MapEnv::new().with_var("DDLINT_CONFIG_PATH", &child_path));
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    Ok((fixture_dir, child_path, discovery, merge))
}

fn load_with_flag<F>(
    flag: &Slot<()>,
    flag_name: &str,
    setup: F,
    extends_context: &ExtendsContext,
) -> Result<()>
where
    F: FnOnce(&Dir) -> Result<()>,
{
    ensure!(flag.is_filled(), "{flag_name} was not initialised");
    flag.clear();
    let (_fixture_dir, child_path, discovery, merge) = prepare_fixture(setup)?;
    let args = [
        OsString::from("prog"),
        OsString::from("--config"),
        child_path.into_os_string(),
    ];
    let result = RulesConfig::load_from_iter_with_sources(args, discovery, merge);
    extends_context.result.set(result);
    Ok(())
}

#[derive(Copy, Clone)]
enum ExtendsScenario {
    Extended,
    Cyclic,
    MissingBase,
    MultiLevel,
    NonString,
}

impl ExtendsScenario {
    fn flag<'a>(&self, context: &'a ExtendsContext) -> &'a Slot<()> {
        match self {
            Self::Extended => &context.extends_flag,
            Self::Cyclic => &context.cyclic_flag,
            Self::MissingBase => &context.missing_base_flag,
            Self::MultiLevel => &context.multi_level_flag,
            Self::NonString => &context.non_string_flag,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Extended => "extended configuration",
            Self::Cyclic => "cyclic configuration",
            Self::MissingBase => "missing-base configuration",
            Self::MultiLevel => "multi-level configuration",
            Self::NonString => "non-string extends configuration",
        }
    }

    fn setup(self, dir: &Dir) -> Result<()> {
        match self {
            Self::Extended => Self::setup_extended(dir),
            Self::Cyclic => Self::setup_cyclic(dir),
            Self::MissingBase => Self::setup_missing_base(dir),
            Self::MultiLevel => Self::setup_multi_level(dir),
            Self::NonString => Self::setup_non_string(dir),
        }
    }

    fn setup_extended(dir: &Dir) -> Result<()> {
        write_fixture(dir, "base.toml", "rules = [\"base\"]")?;
        write_fixture(
            dir,
            ".ddlint.toml",
            "extends = \"base.toml\"\nrules = [\"child\"]",
        )?;
        Ok(())
    }

    fn setup_cyclic(dir: &Dir) -> Result<()> {
        write_fixture(dir, "a.toml", "extends = \"b.toml\"\nrules = [\"a\"]")?;
        write_fixture(dir, "b.toml", "extends = \"a.toml\"\nrules = [\"b\"]")?;
        write_fixture(dir, ".ddlint.toml", "extends = \"a.toml\"")?;
        Ok(())
    }

    fn setup_missing_base(dir: &Dir) -> Result<()> {
        write_fixture(
            dir,
            ".ddlint.toml",
            "extends = \"missing.toml\"\nrules = [\"main\"]",
        )?;
        Ok(())
    }

    fn setup_multi_level(dir: &Dir) -> Result<()> {
        let grandparent = concat!("rules = [\"grandparent\"]\n");
        let parent = concat!("extends = \"grandparent.toml\"\n", "rules = [\"parent\"]\n",);
        let child = concat!("extends = \"parent.toml\"\n", "rules = [\"child\"]\n",);
        write_fixture(dir, "grandparent.toml", grandparent)?;
        write_fixture(dir, "parent.toml", parent)?;
        write_fixture(dir, ".ddlint.toml", child)?;
        Ok(())
    }

    fn setup_non_string(dir: &Dir) -> Result<()> {
        write_fixture(dir, ".ddlint.toml", "extends = 1")?;
        Ok(())
    }
}

fn load_replace_with_fixture<F>(setup: F) -> Result<OrthoResult<ReplaceRulesConfig>>
where
    F: FnOnce(&Dir) -> Result<()>,
{
    let (_fixture_dir, child_path, discovery, merge) = prepare_fixture(setup)?;
    let args = [
        OsString::from("prog"),
        OsString::from("--config-path"),
        child_path.into_os_string(),
    ];
    Ok(ReplaceRulesConfig::load_from_iter_with_sources(
        args, discovery, merge,
    ))
}

fn load_scenario(scenario: ExtendsScenario, context: &ExtendsContext) -> Result<()> {
    load_with_flag(
        scenario.flag(context),
        scenario.name(),
        |dir| scenario.setup(dir),
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

#[when("the non-string extends configuration is loaded")]
fn load_non_string(extends_context: &ExtendsContext) -> Result<()> {
    load_scenario(ExtendsScenario::NonString, extends_context)
}

#[then("an error occurs")]
fn error_occurs(extends_context: &ExtendsContext) -> Result<()> {
    let result = extends_context
        .result
        .take_or("configuration result unavailable")?;
    ensure!(result.is_err(), "expected configuration to fail");
    Ok(())
}

fn strip_rule_quotes(value: &str) -> &str {
    let trimmed = value.trim();
    if let Some(stripped) = trimmed
        .strip_prefix('"')
        .and_then(|val| val.strip_suffix('"'))
    {
        return stripped;
    }
    if let Some(stripped) = trimmed
        .strip_prefix('\'')
        .and_then(|val| val.strip_suffix('\''))
    {
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
        .with_ref(|result| match result {
            Ok(cfg) => Ok(cfg.rules.clone()),
            Err(err) => Err(err.to_string()),
        })
        .ok_or_else(|| anyhow!("configuration result unavailable"))?
        .map_err(anyhow::Error::msg)?;
    let expected = parse_rules_list(&rules);
    ensure!(
        actual == expected,
        "unexpected rules {:?}; expected {:?}",
        actual,
        expected
    );
    Ok(())
}

#[then("the effective rules are {rules}")]
fn rules_are(extends_context: &ExtendsContext, rules: String) -> Result<()> {
    // Check replace_result first if available, otherwise fall back to result
    if let Some(replace_result) = extends_context
        .replace_result
        .with_ref(|result| match result {
            Ok(cfg) => Ok(cfg.rules.clone()),
            Err(err) => Err(err.to_string()),
        })
    {
        let expected = parse_rules_list(&rules);
        let actual = replace_result.map_err(anyhow::Error::msg)?;
        ensure!(
            actual == expected,
            "unexpected rules {:?}; expected {:?}",
            actual,
            expected
        );
        return Ok(());
    }
    inherited_rules(extends_context, rules)
}

#[when("the replace-strategy configuration is loaded")]
fn load_replace_strategy(extends_context: &ExtendsContext) -> Result<()> {
    ensure!(
        extends_context.replace_strategy_flag.is_filled(),
        "replace-strategy configuration was not initialised"
    );
    extends_context.replace_strategy_flag.clear();
    let result = load_replace_with_fixture(|dir| {
        write_fixture(dir, "base.toml", "rules = [\"base\"]")?;
        write_fixture(
            dir,
            ".ddlint.toml",
            "extends = \"base.toml\"\nrules = [\"child\"]",
        )?;
        Ok(())
    })?;
    extends_context.replace_result.set(result);
    Ok(())
}
