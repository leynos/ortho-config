//! Tests for aggregated error reporting across configuration sources.

use ortho_config::{OrthoConfig, OrthoError};
use rstest::rstest;
use serde::Deserialize;

#[derive(Debug, Deserialize, OrthoConfig)]
struct AggConfig {
    #[expect(
        dead_code,
        reason = "Field is read via deserialisation only in this test"
    )]
    port: u32,
}

#[rstest]
fn aggregates_cli_file_env_errors() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "port = ")?; // invalid TOML
        j.set_env("PORT", "notanumber");
        let err = AggConfig::load_from_iter(["prog", "--bogus"])
            .expect_err("expected aggregated error from CLI/file/env sources");
        match err {
            OrthoError::Aggregate(agg) => {
                assert_eq!(agg.len(), 3);
                let mut kinds = agg
                    .iter()
                    .map(|e| match e {
                        OrthoError::CliParsing(_) => 1,
                        OrthoError::File { .. } => 2,
                        OrthoError::Merge { .. } | OrthoError::Gathering(_) => 3,
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
