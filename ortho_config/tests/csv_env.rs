//! Tests for the `CsvEnv` provider.

use figment::Figment;
use ortho_config::CsvEnv;
use rstest::rstest;
use serde::Deserialize;

#[derive(Debug, Deserialize, serde::Serialize)]
struct Cfg {
    values: Vec<String>,
}

#[rstest]
#[case("A,B,C", vec!["A", "B", "C"])]
#[case("[\"x\",\"y\"]", vec!["x", "y"])]
fn parses_lists(#[case] raw: &str, #[case] expected: Vec<&str>) {
    figment::Jail::expect_with(|j| {
        j.set_env("VALUES", raw);
        let cfg: Cfg = Figment::from(CsvEnv::raw()).extract().expect("extract");
        let want: Vec<String> = expected.into_iter().map(str::to_string).collect();
        assert_eq!(cfg.values, want);
        Ok(())
    });
}
