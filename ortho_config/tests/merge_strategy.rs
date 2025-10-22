//! Tests for the append merge strategy on vectors.
#![allow(
    unfulfilled_lint_expectations,
    reason = "clippy::expect_used is denied globally; tests may not hit those branches"
)]
#![expect(
    clippy::expect_used,
    reason = "tests panic to surface configuration mistakes"
)]

use ortho_config::OrthoConfig;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct VecConfig {
    #[ortho_config(merge_strategy = "append")]
    values: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct DefaultVec {
    #[ortho_config(default = vec![String::from("def")], merge_strategy = "append")]
    values: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct EmptyVec {
    #[ortho_config(default = vec![], merge_strategy = "append")]
    values: Vec<String>,
}

#[rstest]
fn append_merges_all_sources() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "values = [\"file\"]")?;
        j.set_env("VALUES", "[\"env\"]");
        let cfg = VecConfig::load_from_iter(["prog", "--values", "cli1", "--values", "cli2"])
            .expect("load");
        assert_eq!(cfg.values, vec!["file", "env", "cli1", "cli2"]);
        Ok(())
    });
}

#[rstest]
fn append_empty_sources_yields_empty() {
    figment::Jail::expect_with(|_| {
        let cfg = EmptyVec::load_from_iter(["prog"]).expect("load");
        assert!(cfg.values.is_empty());
        Ok(())
    });
}

#[rstest]
fn append_includes_defaults() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "values = [\"file\"]")?;
        j.set_env("VALUES", "[\"env\"]");
        let cfg = DefaultVec::load_from_iter(["prog", "--values", "cli"]).expect("load");
        assert_eq!(cfg.values, vec!["def", "file", "env", "cli"]);
        Ok(())
    });
}
