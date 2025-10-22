//! Tests for ignore pattern handling across sources.
use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct IgnoreCfg {
    #[serde(default)]
    #[ortho_config(merge_strategy = "append")]
    ignore_patterns: Vec<String>,
}

fn with_jail<F>(f: F) -> Result<()>
where
    F: FnOnce(&mut figment::Jail) -> Result<()>,
{
    figment::Jail::try_with(|j| f(j).map_err(|err| figment::Error::from(err.to_string())))
        .map_err(|err| anyhow!(err))
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
    with_jail(|j| {
        if let Some(val) = env {
            j.set_env("IGNORE_PATTERNS", val);
        }
        let mut args = vec!["prog"];
        if let Some(val) = cli {
            args.push("--ignore-patterns");
            args.push(val.trim());
        }
        let cfg = IgnoreCfg::load_from_iter(args).map_err(|err| anyhow!(err))?;
        let expected_vec: Vec<String> = expected.into_iter().map(str::to_owned).collect();
        ensure!(
            cfg.ignore_patterns == expected_vec,
            "expected {:?}, got {:?}",
            expected_vec,
            cfg.ignore_patterns
        );
        Ok(())
    })?;
    Ok(())
}
