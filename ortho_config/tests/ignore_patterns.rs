//! Tests for ignore pattern handling across sources.
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
struct IgnoreCfg {
    #[serde(default)]
    #[ortho_config(merge_strategy = "append")]
    ignore_patterns: Vec<String>,
}

#[rstest]
#[case(None, None, vec![])]
#[case(Some(".git/,build/"), None, vec![".git/", "build/"])]
#[case(None, Some("target/"), vec!["target/"])]
#[case(Some(".git/,build/"), Some("target/"), vec![".git/", "build/", "target/"])]
#[case(Some(" .git/ , build/ "), Some(" target/ "), vec![".git/", "build/", "target/"])]
#[case(Some(".git/,.git/"), Some(".git/"), vec![".git/", ".git/", ".git/"])]
fn merges_ignore_patterns_matrix(
    #[case] env: Option<&str>,
    #[case] cli: Option<&str>,
    #[case] expected: Vec<&str>,
) {
    figment::Jail::expect_with(|j| {
        if let Some(val) = env {
            j.set_env("IGNORE_PATTERNS", val);
        }
        let mut args = vec!["prog"];
        if let Some(val) = cli {
            args.push("--ignore-patterns");
            args.push(val.trim());
        }
        let cfg = IgnoreCfg::load_from_iter(args).expect("load");
        assert_eq!(cfg.ignore_patterns, expected);
        Ok(())
    });
}
