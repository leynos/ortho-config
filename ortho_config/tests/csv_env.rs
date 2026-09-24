//! Unit tests for the `CsvEnv` provider.
//!
//! Ensure that comma-separated environment variables are parsed into arrays
//! and that existing JSON strings remain intact.

use anyhow::{Context, Result, anyhow, ensure};
use figment::Figment;
use ortho_config::{CsvEnv, MapEnv};
use rstest::rstest;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, serde::Serialize)]
struct Cfg {
    values: Vec<String>,
}

#[rstest]
#[case("A,B,C", vec!["A", "B", "C"])]
#[case("[\"x\",\"y\"]", vec!["x", "y"])]
#[case("A, B, C", vec!["A", "B", "C"])]
#[case("A,B,", vec!["A", "B", ""])]
#[case(",A,B", vec!["", "A", "B"])]
fn parses_lists(#[case] raw: &str, #[case] expected: Vec<&str>) -> Result<()> {
    let want: Vec<String> = expected.into_iter().map(str::to_string).collect();
    let source = Arc::new(MapEnv::new().with_var("VALUES", raw));
    let cfg: Cfg = Figment::from(CsvEnv::raw().with_source(source))
        .extract()
        .context("failed to extract Cfg from CsvEnv")?;
    ensure!(
        cfg.values == want,
        "expected {:?}, got {:?}",
        want,
        cfg.values
    );
    Ok(())
}

#[rstest]
#[case("")]
#[case("single")]
fn fails_on_non_lists(#[case] raw: &str) -> Result<()> {
    let source = Arc::new(MapEnv::new().with_var("VALUES", raw));
    match Figment::from(CsvEnv::raw().with_source(source)).extract::<Cfg>() {
        Ok(cfg) => Err(anyhow!(
            "expected parse failure for {raw:?}, but succeeded with values {:?}",
            cfg.values
        )),
        Err(_) => Ok(()),
    }
}

#[derive(Debug, Deserialize)]
struct BoolCfg {
    flag: bool,
}

#[rstest]
#[case("true", true)]
#[case("false", false)]
#[case("TRUE", true)]
#[case("FALSE", false)]
#[case("True", true)]
#[case("False", false)]
#[case("  true  ", true)]
#[case("  false  ", false)]
fn parses_booleans(#[case] raw: &str, #[case] expected: bool) -> Result<()> {
    let source = Arc::new(MapEnv::new().with_var("FLAG", raw));
    let cfg: BoolCfg = Figment::from(CsvEnv::raw().with_source(source))
        .extract()
        .context("failed to extract BoolCfg from CsvEnv")?;
    ensure!(
        cfg.flag == expected,
        "expected {expected}, got {}",
        cfg.flag
    );
    Ok(())
}
