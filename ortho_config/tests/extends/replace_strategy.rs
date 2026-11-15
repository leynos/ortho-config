//! Replacement semantics for fields declared inside an `extends` chain.
//!
//! The default merge for a sequence field appends across the chain, so a test
//! that only exercises the append path cannot tell a working
//! `merge_strategy = "replace"` from one that was ignored. These cases pin the
//! replacement contract at each chain depth, including the empty-array case
//! that a truthiness-based implementation would silently skip.

use anyhow::{Result, anyhow};
use ortho_config::{MapEnv, OrthoConfig};
use rstest::rstest;
use serde::{Deserialize, Serialize};

use super::extends_fixture::ConfigFixture;
use super::profile_chain_assertions::ensure_eq;

/// Config struct with explicit replace strategy for the tags field.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct ReplaceTagsCfg {
    #[serde(default)]
    #[ortho_config(merge_strategy = "replace")]
    tags: Vec<String>,
}

/// One inheritance shape and the tags the merged configuration must report.
#[derive(Debug)]
struct ReplaceStrategyCase {
    files: Vec<(&'static str, &'static str)>,
    expected_tags: Vec<&'static str>,
}

/// Verifies that `merge_strategy = "replace"` works correctly with extends.
#[rstest]
#[case::single_level(ReplaceStrategyCase {
    files: vec![
        ("parent.toml", "tags = [\"parent\"]"),
        (".config.toml", "extends = \"parent.toml\"\ntags = [\"child\"]"),
    ],
    expected_tags: vec!["child"],
})]
#[case::multi_level_chain(ReplaceStrategyCase {
    files: vec![
        ("grandparent.toml", "tags = [\"grandparent\"]"),
        ("parent.toml", "extends = \"grandparent.toml\"\ntags = [\"parent\"]"),
        (".config.toml", "extends = \"parent.toml\"\ntags = [\"child\"]"),
    ],
    expected_tags: vec!["child"],
})]
#[case::empty_array(ReplaceStrategyCase {
    files: vec![
        ("parent.toml", "tags = [\"parent\"]"),
        (".config.toml", "extends = \"parent.toml\"\ntags = []"),
    ],
    expected_tags: vec![],
})]
fn extends_with_replace_strategy_behaviour(#[case] case: ReplaceStrategyCase) -> Result<()> {
    let fixture = ConfigFixture::new()?;
    for (filename, content) in case.files {
        fixture.create_file(filename, content)?;
    }
    let cfg: ReplaceTagsCfg = fixture
        .load(["prog"], MapEnv::new())
        .map_err(|err| anyhow!(err))?;
    let expected: Vec<String> = case.expected_tags.into_iter().map(String::from).collect();
    ensure_eq(&cfg.tags, &expected, "tags")
}
