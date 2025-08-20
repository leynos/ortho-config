//! Tests for ignore pattern handling across sources.

use ortho_config::OrthoConfig;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct IgnoreCfg {
    #[serde(default)]
    #[ortho_config(merge_strategy = "append")]
    ignore_patterns: Vec<String>,
}

#[rstest]
fn merges_ignore_patterns() {
    figment::Jail::expect_with(|j| {
        j.set_env("IGNORE_PATTERNS", ".git/,build/");
        let cfg =
            IgnoreCfg::load_from_iter(["prog", "--ignore-patterns", "target/"]).expect("load");
        assert_eq!(cfg.ignore_patterns, vec![".git/", "build/", "target/"]);
        Ok(())
    });
}
