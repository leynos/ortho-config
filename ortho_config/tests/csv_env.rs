//! Unit tests for the `CsvEnv` provider.
//!
//! Ensure that comma-separated environment variables are parsed into arrays
//! and that existing JSON strings remain intact.
#![allow(
    unfulfilled_lint_expectations,
    reason = "clippy::expect_used is denied globally; tests may not hit those branches"
)]
#![expect(
    clippy::expect_used,
    reason = "tests panic to surface configuration mistakes"
)]

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
#[case("A, B, C", vec!["A", "B", "C"])]
#[case("A,B,", vec!["A", "B", ""])]
#[case(",A,B", vec!["", "A", "B"])]
fn parses_lists(#[case] raw: &str, #[case] expected: Vec<&str>) {
    figment::Jail::expect_with(|j| {
        j.set_env("VALUES", raw);
        let cfg: Cfg = Figment::from(CsvEnv::raw()).extract().expect("extract");
        let want: Vec<String> = expected.into_iter().map(str::to_string).collect();
        assert_eq!(cfg.values, want);
        Ok(())
    });
}

#[rstest]
#[case("")]
#[case("single")]
fn fails_on_non_lists(#[case] raw: &str) {
    figment::Jail::expect_with(|j| {
        j.set_env("VALUES", raw);
        Figment::from(CsvEnv::raw())
            .extract::<Cfg>()
            .expect_err("non-list inputs must fail to parse");
        Ok(())
    });
}
