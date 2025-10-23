//! Tests covering configuration path resolution across CLI and environment.

use super::common::{assert_config_values, with_jail, OrthoResultExt, RenamedPathConfig, TestConfig};
use anyhow::{ensure, Result};
use rstest::rstest;

struct ConfigPathCase {
    files: &'static [(&'static str, &'static str)],
    env: &'static [(&'static str, &'static str)],
    cli_args: &'static [&'static str],
    expected_sample: Option<&'static str>,
    expected_other: Option<&'static str>,
}

#[rstest]
#[case::env_override(ConfigPathCase {
    files: &[ ("alt.toml", "sample_value = \"from_env\"\nother = \"val\"") ],
    env: &[ ("CONFIG_PATH", "alt.toml") ],
    cli_args: &["prog"],
    expected_sample: Some("from_env"),
    expected_other: Some("val"),
})]
#[case::cli_overrides_default(ConfigPathCase {
    files: &[
        (".config.toml", "sample_value = \"default\"\nother = \"d\""),
        ("alt.toml", "sample_value = \"alt\"\nother = \"a\""),
    ],
    env: &[],
    cli_args: &["prog", "--config-path", "alt.toml"],
    expected_sample: Some("alt"),
    expected_other: Some("a"),
})]
#[case::missing_file_is_ignored(ConfigPathCase {
    files: &[],
    env: &[ ("CONFIG_PATH", "nope.toml") ],
    cli_args: &["prog", "--sample-value", "cli", "--other", "val"],
    expected_sample: Some("cli"),
    expected_other: Some("val"),
})]
fn resolves_config_path_priorities(#[case] case: ConfigPathCase) -> Result<()> {
    with_jail(|j| {
        for (path, contents) in case.files {
            j.create_file(path, contents)?;
        }
        for (key, value) in case.env {
            j.set_env(key, value);
        }
        let cfg = TestConfig::load_from_iter(case.cli_args.iter().copied()).to_anyhow()?;
        assert_config_values(&cfg, case.expected_sample, case.expected_other)
    })
}

struct RenamedPathCase {
    files: &'static [(&'static str, &'static str)],
    env: &'static [(&'static str, &'static str)],
    cli_args: &'static [&'static str],
    expected_sample: &'static str,
}

#[rstest]
#[case::custom_flag(RenamedPathCase {
    files: &[ ("alt.toml", "sample = \"file\"") ],
    env: &[],
    cli_args: &["prog", "--config", "alt.toml"],
    expected_sample: "file",
})]
#[case::custom_env(RenamedPathCase {
    files: &[ ("alt.toml", "sample = \"env\"") ],
    env: &[ ("CONFIG_PATH", "alt.toml") ],
    cli_args: &["prog"],
    expected_sample: "env",
})]
#[case::cli_overrides_env(RenamedPathCase {
    files: &[
        ("env.toml", "sample = \"env\""),
        ("cli.toml", "sample = \"cli\""),
    ],
    env: &[ ("CONFIG_PATH", "env.toml") ],
    cli_args: &["prog", "--config", "cli.toml"],
    expected_sample: "cli",
})]
fn resolves_custom_config_flag(#[case] case: RenamedPathCase) -> Result<()> {
    with_jail(|j| {
        for (path, contents) in case.files {
            j.create_file(path, contents)?;
        }
        for (key, value) in case.env {
            j.set_env(key, value);
        }
        let cfg = RenamedPathConfig::load_from_iter(case.cli_args.iter().copied()).to_anyhow()?;
        ensure!(
            cfg.sample.as_deref() == Some(case.expected_sample),
            "expected sample {}, got {:?}",
            case.expected_sample,
            cfg.sample
        );
        ensure!(
            cfg.config_path.is_none(),
            "config_path should not be retained post-merge"
        );
        Ok(())
    })
}
