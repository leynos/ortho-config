//! Behavioural tests covering discovery error buffering.

use ortho_config::{OrthoConfig, OrthoError};
use rstest::rstest;
use serde::Deserialize;

#[derive(Debug, Deserialize, OrthoConfig)]
struct BufferingConfig {
    value: Option<String>,
}

#[rstest]
fn ignores_buffered_errors_when_fallback_succeeds() {
    figment::Jail::expect_with(|j| {
        j.create_file("broken.toml", "value = [")?; // invalid TOML to trigger an error
        j.set_env("CONFIG_PATH", "broken.toml");
        j.create_file(".config.toml", "value = \"from_file\"")?;

        let cfg = BufferingConfig::load_from_iter(["prog"]).expect("load succeeds");
        assert_eq!(cfg.value.as_deref(), Some("from_file"));
        Ok(())
    });
}

#[rstest]
fn surfaces_buffered_errors_when_no_config_found() {
    figment::Jail::expect_with(|j| {
        j.create_file("broken.toml", "value = [")?; // invalid TOML to trigger an error
        j.set_env("CONFIG_PATH", "broken.toml");

        let err = BufferingConfig::load_from_iter(["prog"]).expect_err("expected failure");
        match &*err {
            OrthoError::File { path, .. } => {
                assert!(path.ends_with("broken.toml"));
            }
            other => panic!("expected file error, got {other:?}"),
        }
        Ok(())
    });
}
