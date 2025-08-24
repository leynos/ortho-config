//! Tests for configuration inheritance using the `extends` key.

use ortho_config::{OrthoConfig, OrthoError};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct ExtendsCfg {
    #[serde(skip_serializing_if = "Option::is_none")]
    foo: Option<String>,
}

#[rstest]
fn extended_file_overrides_base() {
    figment::Jail::expect_with(|j| {
        j.create_file("base.toml", "foo = \"base\"")?;
        j.create_file(".config.toml", "extends = \"base.toml\"\nfoo = \"child\"")?;
        let cfg = ExtendsCfg::load_from_iter(["prog"]).expect("load");
        assert_eq!(cfg.foo.as_deref(), Some("child"));
        Ok(())
    });
}

#[rstest]
fn env_and_cli_override_extended_file() {
    figment::Jail::expect_with(|j| {
        j.create_file("base.toml", "foo = \"base\"")?;
        j.create_file(".config.toml", "extends = \"base.toml\"\nfoo = \"file\"")?;
        j.set_env("FOO", "env");
        let cfg = ExtendsCfg::load_from_iter(["prog", "--foo", "cli"]).expect("load");
        assert_eq!(cfg.foo.as_deref(), Some("cli"));
        Ok(())
    });
}

#[rstest]
fn cyclic_inheritance_is_detected() {
    figment::Jail::expect_with(|j| {
        j.create_file("a.toml", "extends = \"b.toml\"\nfoo = \"a\"")?;
        j.create_file("b.toml", "extends = \"a.toml\"\nfoo = \"b\"")?;
        j.create_file(".config.toml", "extends = \"a.toml\"")?;
        let err = ExtendsCfg::load_from_iter(["prog"]).unwrap_err();
        assert!(matches!(err, OrthoError::CyclicExtends { .. }));
        Ok(())
    });
}

#[rstest]
fn missing_base_file_errors() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "extends = \"missing.toml\"")?;
        let err = ExtendsCfg::load_from_iter(["prog"]).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("missing.toml"));
        Ok(())
    });
}

#[rstest]
fn non_string_extends_errors() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "extends = 1")?;
        let err = ExtendsCfg::load_from_iter(["prog"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("must be a string"));
        // Also assert the origin file is mentioned for better diagnostics.
        assert!(msg.contains(".config.toml"));
        Ok(())
    });
}

#[rstest]
fn empty_extends_errors() {
    figment::Jail::expect_with(|j| {
        j.create_file("base.toml", "")?; // placeholder so Jail has root file
        j.create_file(".config.toml", "extends = ''")?;
        let err = ExtendsCfg::load_from_iter(["prog"]).unwrap_err();
        assert!(err.to_string().contains("not a regular file"));
        Ok(())
    });
}

#[rstest]
fn directory_extends_errors() {
    figment::Jail::expect_with(|j| {
        j.create_dir("dir")?;
        j.create_file(".config.toml", "extends = 'dir'")?;
        let err = ExtendsCfg::load_from_iter(["prog"]).unwrap_err();
        assert!(err.to_string().contains("not a regular file"));
        Ok(())
    });
}
