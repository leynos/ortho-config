//! Tests for aggregated error reporting across configuration sources.

use ortho_config::{OrthoConfig, OrthoError};
use rstest::rstest;
use serde::Deserialize;

#[derive(Debug, Deserialize, OrthoConfig)]
struct AggConfig {
    #[allow(dead_code)]
    port: u32,
}

#[rstest]
fn aggregates_cli_file_env_errors() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "port = ")?; // invalid TOML
        j.set_env("PORT", "notanumber");
        let err = AggConfig::load_from_iter(["prog", "--bogus"]).unwrap_err();
        match err {
            OrthoError::Aggregate(agg) => {
                assert_eq!(agg.len(), 3);
                let mut kinds = agg
                    .iter()
                    .map(|e| match e {
                        OrthoError::CliParsing(_) => 1,
                        OrthoError::File { .. } => 2,
                        OrthoError::Gathering(_) => 3,
                        _ => 0,
                    })
                    .collect::<Vec<_>>();
                kinds.sort_unstable();
                assert_eq!(kinds, vec![1, 2, 3]);
            }
            other => panic!("unexpected error: {other:?}"),
        }
        Ok(())
    });
}
