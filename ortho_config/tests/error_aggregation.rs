//! Tests for aggregated error reporting across configuration sources.
#![allow(
    unfulfilled_lint_expectations,
    reason = "clippy::expect_used is denied globally; tests may not hit those branches"
)]
#![expect(
    clippy::expect_used,
    reason = "tests panic to surface configuration mistakes"
)]

use ortho_config::{OrthoConfig, OrthoError};
use rstest::rstest;
use serde::Deserialize;

#[derive(Debug, Deserialize, OrthoConfig)]
struct AggConfig {
    #[expect(
        dead_code,
        reason = "Field is read via deserialization only in this test"
    )]
    port: u32,
}

#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(
    prefix = "AGG_",
    discovery(
        app_name = "agg_config",
        env_var = "AGG_CONFIG_PATH",
        dotfile_name = ".agg.toml"
    )
)]
struct DiscoveryErrorConfig {
    #[ortho_config(default = 0)]
    port: u32,
}

#[rstest]
fn aggregates_cli_file_env_errors() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "port = ")?; // invalid TOML
        j.set_env("PORT", "notanumber");
        let err = AggConfig::load_from_iter(["prog", "--bogus"])
            .expect_err("expected aggregated error from CLI/file/env sources");
        match &*err {
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

#[rstest]
fn discovery_errors_hidden_when_fallback_succeeds() {
    figment::Jail::expect_with(|j| {
        j.create_file("invalid.toml", "port = ???")?;
        j.create_file(".agg.toml", "port = 7000")?;
        j.set_env("AGG_CONFIG_PATH", "invalid.toml");

        let cfg = DiscoveryErrorConfig::load_from_iter(["prog"])
            .expect("expected fallback discovery to succeed");
        assert_eq!(cfg.port, 7000);
        Ok(())
    });
}

#[rstest]
fn required_path_errors_surface_even_with_fallback() {
    figment::Jail::expect_with(|j| {
        j.create_file(".agg.toml", "port = 7000")?;

        let err = DiscoveryErrorConfig::load_from_iter(["prog", "--config-path", "missing.toml"])
            .expect_err("expected missing required CLI path to error");
        assert!(matches!(&*err, OrthoError::File { .. }));
        Ok(())
    });
}

#[rstest]
fn discovery_errors_surface_when_all_candidates_fail() {
    figment::Jail::expect_with(|j| {
        j.create_file("invalid.toml", "port = ???")?;
        j.set_env("AGG_CONFIG_PATH", "invalid.toml");

        let err = DiscoveryErrorConfig::load_from_iter(["prog"]) // no fallback file
            .expect_err("expected discovery error when no file loads");
        assert!(matches!(&*err, OrthoError::File { .. }));
        Ok(())
    });
}
