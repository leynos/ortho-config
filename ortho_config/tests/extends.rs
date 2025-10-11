//! Tests for configuration inheritance using the `extends` key.

use ortho_config::{OrthoConfig, OrthoError};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[inline]
#[allow(deprecated, reason = "figment::Jail is used for test isolation only")]
fn with_jail<F>(f: F)
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
{
    figment::Jail::expect_with(f);
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct ExtendsCfg {
    #[serde(skip_serializing_if = "Option::is_none")]
    foo: Option<String>,
}

#[rstest]
#[case(
    "base",
    "child",
    &[] as &[&str],
    None,
    "child",
)]
#[case(
    "base",
    "file",
    &["--foo", "cli"],
    Some("env"),
    "cli",
)]
fn inheritance_precedence(
    #[case] base_value: &str,
    #[case] config_value: &str,
    #[case] cli_args: &[&str],
    #[case] env_value: Option<&str>,
    #[case] expected: &str,
) {
    with_jail(|j| {
        j.create_file("base.toml", &format!("foo = \"{base_value}\""))?;
        j.create_file(
            ".config.toml",
            &format!("extends = \"base.toml\"\nfoo = \"{config_value}\""),
        )?;
        if let Some(val) = env_value {
            j.set_env("FOO", val);
        }
        let mut args = vec!["prog"];
        args.extend_from_slice(cli_args);
        let cfg = ExtendsCfg::load_from_iter(args).expect("load");
        assert_eq!(cfg.foo.as_deref(), Some(expected));
        Ok(())
    });
}

#[rstest]
fn cyclic_inheritance_is_detected() {
    with_jail(|j| {
        j.create_file("a.toml", "extends = \"b.toml\"\nfoo = \"a\"")?;
        j.create_file("b.toml", "extends = \"a.toml\"\nfoo = \"b\"")?;
        j.create_file(".config.toml", "extends = \"a.toml\"")?;
        let err = ExtendsCfg::load_from_iter(["prog"]).unwrap_err();
        assert!(matches!(&*err, OrthoError::CyclicExtends { .. }));
        Ok(())
    });
}

#[rstest]
#[cfg_attr(
    not(any(windows, target_os = "macos")),
    ignore = "case-insensitive cycle detection requires Windows or macOS"
)]
fn cyclic_inheritance_detects_case_variants() {
    with_jail(|j| {
        j.create_file("Base.toml", "extends = \".CONFIG.toml\"\nfoo = \"base\"")?;
        j.create_file(".config.toml", "extends = \"base.toml\"\nfoo = \"config\"")?;
        let err = ExtendsCfg::load_from_iter(["prog"]).unwrap_err();
        assert!(matches!(&*err, OrthoError::CyclicExtends { .. }));
        let msg = err.to_string();
        assert!(msg.to_ascii_lowercase().contains("base.toml"));
        assert!(msg.to_ascii_lowercase().contains(".config.toml"));
        Ok(())
    });
}

#[rstest]
fn missing_base_file_errors() {
    with_jail(|j| {
        j.create_file(".config.toml", "extends = \"missing.toml\"")?;
        let err = ExtendsCfg::load_from_iter(["prog"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("missing.toml"));
        Ok(())
    });
}

#[rstest]
fn non_string_extends_errors() {
    with_jail(|j| {
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
    with_jail(|j| {
        j.create_file("base.toml", "")?; // placeholder so Jail has root file
        j.create_file(".config.toml", "extends = ''")?;
        let err = ExtendsCfg::load_from_iter(["prog"]).unwrap_err();
        assert!(err.to_string().contains("non-empty"));
        Ok(())
    });
}

#[rstest]
fn directory_extends_errors() {
    with_jail(|j| {
        j.create_dir("dir")?;
        j.create_file(".config.toml", "extends = 'dir'")?;
        let err = ExtendsCfg::load_from_iter(["prog"]).unwrap_err();
        assert!(err.to_string().contains("not a regular file"));
        Ok(())
    });
}
