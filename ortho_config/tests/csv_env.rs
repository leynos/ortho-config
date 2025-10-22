//! Unit tests for the `CsvEnv` provider.
//!
//! Ensure that comma-separated environment variables are parsed into arrays
//! and that existing JSON strings remain intact.

use anyhow::{Result, anyhow, ensure};
use figment::Figment;
use ortho_config::CsvEnv;
use rstest::rstest;
use serde::Deserialize;

#[derive(Debug, Deserialize, serde::Serialize)]
struct Cfg {
    values: Vec<String>,
}

fn with_jail<F>(f: F) -> Result<()>
where
    F: FnOnce(&mut figment::Jail) -> Result<()>,
{
    figment::Jail::try_with(|j| f(j).map_err(|err| figment::Error::from(err.to_string())))
        .map_err(|err| anyhow!(err))
}

#[rstest]
#[case("A,B,C", vec!["A", "B", "C"])]
#[case("[\"x\",\"y\"]", vec!["x", "y"])]
#[case("A, B, C", vec!["A", "B", "C"])]
#[case("A,B,", vec!["A", "B", ""])]
#[case(",A,B", vec!["", "A", "B"])]
fn parses_lists(#[case] raw: &str, #[case] expected: Vec<&str>) -> Result<()> {
    let want: Vec<String> = expected.into_iter().map(str::to_string).collect();
    with_jail(|j| {
        j.set_env("VALUES", raw);
        let cfg: Cfg = Figment::from(CsvEnv::raw())
            .extract()
            .map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.values == want,
            "expected {:?}, got {:?}",
            want,
            cfg.values
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
#[case("")]
#[case("single")]
fn fails_on_non_lists(#[case] raw: &str) -> Result<()> {
    with_jail(|j| {
        j.set_env("VALUES", raw);
        match Figment::from(CsvEnv::raw()).extract::<Cfg>() {
            Ok(cfg) => Err(anyhow!(
                "expected parse failure for {raw:?}, but succeeded with values {:?}",
                cfg.values
            )),
            Err(_) => Ok(()),
        }
    })?;
    Ok(())
}
