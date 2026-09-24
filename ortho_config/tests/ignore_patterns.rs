//! Tests for ignore pattern handling across sources.
use anyhow::{Result, anyhow, ensure};
use ortho_config::{MapEnv, OrthoConfig};
use rstest::rstest;
use serde::{Deserialize, Serialize};

use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct IgnoreCfg {
    #[serde(default)]
    #[ortho_config(default = Vec::<String>::new(), merge_strategy = "append")]
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
) -> Result<()> {
    let mut env_map = MapEnv::new();
    if let Some(value) = env {
        env_map.insert("IGNORE_PATTERNS", value);
    }
    let source = Arc::new(env_map);
    let mut args = vec!["prog"];
    if let Some(value) = cli {
        args.push("--ignore-patterns");
        args.push(value.trim());
    }
    let cfg = IgnoreCfg::load_from_iter_with_sources(args, source.clone(), source)
        .map_err(|err| anyhow!(err))?;
    let expected_vec: Vec<String> = expected.into_iter().map(str::to_owned).collect();
    ensure!(
        cfg.ignore_patterns == expected_vec,
        "expected {:?}, got {:?}",
        expected_vec,
        cfg.ignore_patterns
    );
    Ok(())
}
