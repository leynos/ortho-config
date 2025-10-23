//! Tests covering CLI integration and error handling.
use anyhow::{Result, anyhow, ensure};
use ortho_config::{OrthoConfig, OrthoError};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct TestConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    other: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct OptionConfig {
    maybe: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct RequiredConfig {
    sample_value: String,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct ConflictConfig {
    second: Option<String>,
    sample: Option<String>,
}

fn with_jail<F>(f: F) -> Result<()>
where
    F: FnOnce(&mut figment::Jail) -> Result<()>,
{
    figment::Jail::try_with(|j| f(j).map_err(|err| figment::Error::from(err.to_string())))
        .map_err(|err| anyhow!(err))
}

fn assert_config_values(
    config: &TestConfig,
    expected_sample: Option<&str>,
    expected_other: Option<&str>,
) -> Result<()> {
    ensure!(
        config.sample_value.as_deref() == expected_sample,
        "expected sample_value {:?}, got {:?}",
        expected_sample,
        config.sample_value
    );
    ensure!(
        config.other.as_deref() == expected_other,
        "expected other {:?}, got {:?}",
        expected_other,
        config.other
    );
    Ok(())
}

#[rstest]
fn parses_kebab_case_flags() -> Result<()> {
    let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "hello", "--other", "val"])
        .map_err(|err| anyhow!(err))?;
    assert_config_values(&cfg, Some("hello"), Some("val"))
}

#[rstest]
fn short_flags_work() -> Result<()> {
    let cfg = TestConfig::load_from_iter(["prog", "-s", "hi", "-o", "val"])
        .map_err(|err| anyhow!(err))?;
    assert_config_values(&cfg, Some("hi"), Some("val"))
}

#[rstest]
fn cli_only_source() -> Result<()> {
    let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "only", "--other", "x"])
        .map_err(|err| anyhow!(err))?;
    assert_config_values(&cfg, Some("only"), Some("x"))
}

#[rstest]
fn cli_overrides_other_sources() -> Result<()> {
    with_jail(|j| {
        j.create_file(".config.toml", "sample_value = \"file\"\nother = \"f\"")?;
        j.set_env("SAMPLE_VALUE", "env");
        j.set_env("OTHER", "e");
        let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "cli", "--other", "cli2"])
            .map_err(|err| anyhow!(err))?;
        assert_config_values(&cfg, Some("cli"), Some("cli2"))
    })?;
    Ok(())
}

#[rstest]
fn cli_combines_with_file() -> Result<()> {
    with_jail(|j| {
        j.create_file(".config.toml", "other = \"file\"")?;
        let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "cli", "--other", "cli2"])
            .map_err(|err| anyhow!(err))?;
        assert_config_values(&cfg, Some("cli"), Some("cli2"))
    })?;
    Ok(())
}

#[rstest]
fn invalid_cli_input_maps_error() -> Result<()> {
    let err = match TestConfig::load_from_iter(["prog", "--bogus"]) {
        Ok(cfg) => return Err(anyhow!("expected CLI parsing error, got config {:?}", cfg)),
        Err(err) => err,
    };
    ensure!(
        matches!(&*err, OrthoError::CliParsing(_)),
        "expected CLI parsing error, got {:?}",
        err
    );
    Ok(())
}

#[rstest]
fn invalid_cli_wrong_type_maps_error() -> Result<()> {
    let err = match OptionConfig::load_from_iter(["prog", "--maybe", "notanumber"]) {
        Ok(cfg) => {
            return Err(anyhow!(
                "expected CLI parsing failure, got config {:?}",
                cfg
            ));
        }
        Err(err) => err,
    };
    ensure!(
        matches!(&*err, OrthoError::CliParsing(_)),
        "expected CLI parsing error, got {:?}",
        err
    );
    Ok(())
}

#[rstest]
fn invalid_cli_missing_required_maps_error() -> Result<()> {
    with_jail(|_| match RequiredConfig::load_from_iter(["prog"]) {
        Ok(cfg) => Err(anyhow!(
            "expected merge error for missing config, got {:?}",
            cfg
        )),
        Err(err) => {
            ensure!(
                matches!(&*err, OrthoError::Merge { .. }),
                "expected merge error, got {:?}",
                err
            );
            Ok(())
        }
    })?;
    Ok(())
}

#[rstest]
fn invalid_cli_duplicate_flag_maps_error() -> Result<()> {
    let err = match TestConfig::load_from_iter([
        "prog",
        "--sample-value",
        "foo",
        "--sample-value",
        "bar",
    ]) {
        Ok(cfg) => return Err(anyhow!("expected CLI parsing error, got config {:?}", cfg)),
        Err(err) => err,
    };
    ensure!(
        matches!(&*err, OrthoError::CliParsing(_)),
        "expected CLI parsing error, got {:?}",
        err
    );
    Ok(())
}

#[rstest]
fn merges_cli_into_figment() -> Result<()> {
    use figment::{Figment, Profile, providers::Serialized};

    let cli = TestConfig {
        sample_value: Some("hi".into()),
        other: Some("there".into()),
    };

    let cfg: TestConfig = Figment::new()
        .merge(Serialized::from(cli, Profile::Default))
        .extract()
        .map_err(|err| anyhow!(err))?;

    ensure!(
        cfg.sample_value.as_deref() == Some("hi"),
        "expected sample_value hi, got {:?}",
        cfg.sample_value
    );
    ensure!(
        cfg.other.as_deref() == Some("there"),
        "expected other there, got {:?}",
        cfg.other
    );
    Ok(())
}

#[rstest]
fn option_field_cli_present() -> Result<()> {
    let cfg = OptionConfig::load_from_iter(["prog", "--maybe", "5"]).map_err(|err| anyhow!(err))?;
    ensure!(
        cfg.maybe == Some(5),
        "expected maybe 5, got {:?}",
        cfg.maybe
    );
    Ok(())
}

#[rstest]
fn option_field_cli_absent() -> Result<()> {
    let cfg = OptionConfig::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
    ensure!(cfg.maybe.is_none(), "expected None, got {:?}", cfg.maybe);
    Ok(())
}

#[rstest]
fn resolves_short_flag_conflict() -> Result<()> {
    let cfg = ConflictConfig::load_from_iter(["prog", "-s", "one", "-S", "two"])
        .map_err(|err| anyhow!(err))?;
    ensure!(
        cfg.second.as_deref() == Some("one"),
        "expected second one, got {:?}",
        cfg.second
    );
    ensure!(
        cfg.sample.as_deref() == Some("two"),
        "expected sample two, got {:?}",
        cfg.sample
    );
    Ok(())
}

#[rstest]
fn config_path_env_var() -> Result<()> {
    with_jail(|j| {
        j.create_file("alt.toml", "sample_value = \"from_env\"\nother = \"val\"")?;
        j.set_env("CONFIG_PATH", "alt.toml");

        let cfg = TestConfig::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.sample_value.as_deref() == Some("from_env"),
            "expected sample_value from_env, got {:?}",
            cfg.sample_value
        );
        ensure!(
            cfg.other.as_deref() == Some("val"),
            "expected other val, got {:?}",
            cfg.other
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn config_path_cli_overrides_default() -> Result<()> {
    with_jail(|j| {
        j.create_file(".config.toml", "sample_value = \"default\"\nother = \"d\"")?;
        j.create_file("alt.toml", "sample_value = \"alt\"\nother = \"a\"")?;

        let cfg = TestConfig::load_from_iter(["prog", "--config-path", "alt.toml"])
            .map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.sample_value.as_deref() == Some("alt"),
            "expected sample_value alt, got {:?}",
            cfg.sample_value
        );
        ensure!(
            cfg.other.as_deref() == Some("a"),
            "expected other a, got {:?}",
            cfg.other
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn missing_config_file_is_ignored() -> Result<()> {
    with_jail(|j| {
        j.set_env("CONFIG_PATH", "nope.toml");

        let cfg = TestConfig::load_from_iter(["prog", "--sample-value", "cli", "--other", "val"])
            .map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.sample_value.as_deref() == Some("cli"),
            "expected sample_value cli, got {:?}",
            cfg.sample_value
        );
        ensure!(
            cfg.other.as_deref() == Some("val"),
            "expected other val, got {:?}",
            cfg.other
        );
        Ok(())
    })?;
    Ok(())
}

// Windows lacks XDG support
#[cfg(any(unix, target_os = "redox"))]
#[test]
fn loads_from_xdg_config() -> Result<()> {
    with_jail(|j| {
        let dir = j.create_dir("xdg")?;
        let abs = ortho_config::file::canonicalise(&dir).map_err(|err| anyhow!(err))?;
        j.create_file(
            dir.join("config.toml"),
            "sample_value = \"xdg\"\nother = \"val\"",
        )?;
        let dir_value = abs
            .to_str()
            .ok_or_else(|| anyhow!("canonical path is not valid UTF-8: {:?}", abs))?
            .to_owned();
        j.set_env("XDG_CONFIG_HOME", &dir_value);

        let cfg = TestConfig::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.sample_value.as_deref() == Some("xdg"),
            "expected sample_value xdg, got {:?}",
            cfg.sample_value
        );
        ensure!(
            cfg.other.as_deref() == Some("val"),
            "expected other val, got {:?}",
            cfg.other
        );
        Ok(())
    })?;
    Ok(())
}

// Windows lacks XDG support
#[cfg(any(unix, target_os = "redox"))]
#[cfg(feature = "yaml")]
#[test]
fn loads_from_xdg_yaml_config() -> Result<()> {
    with_jail(|j| {
        let dir = j.create_dir("xdg_yaml")?;
        let abs = ortho_config::file::canonicalise(&dir).map_err(|err| anyhow!(err))?;
        j.create_file(dir.join("config.yaml"), "sample_value: xdg\nother: val")?;
        let dir_value = abs
            .to_str()
            .ok_or_else(|| anyhow!("canonical path is not valid UTF-8: {:?}", abs))?
            .to_owned();
        j.set_env("XDG_CONFIG_HOME", &dir_value);

        let cfg = TestConfig::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.sample_value.as_deref() == Some("xdg"),
            "expected sample_value xdg, got {:?}",
            cfg.sample_value
        );
        ensure!(
            cfg.other.as_deref() == Some("val"),
            "expected other val, got {:?}",
            cfg.other
        );
        Ok(())
    })?;
    Ok(())
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct RenamedPathConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    sample: Option<String>,
    #[serde(skip)]
    #[ortho_config(cli_long = "config")]
    config_path: Option<std::path::PathBuf>,
}

#[rstest]
fn config_path_custom_flag() -> Result<()> {
    with_jail(|j| {
        j.create_file("alt.toml", "sample = \"file\"")?;
        let cfg = RenamedPathConfig::load_from_iter(["prog", "--config", "alt.toml"])
            .map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.sample.as_deref() == Some("file"),
            "expected sample file, got {:?}",
            cfg.sample
        );
        ensure!(
            cfg.config_path.is_none(),
            "config_path should not be retained post-merge"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn config_path_custom_env() -> Result<()> {
    with_jail(|j| {
        j.create_file("alt.toml", "sample = \"env\"")?;
        j.set_env("CONFIG_PATH", "alt.toml");
        let cfg = RenamedPathConfig::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.sample.as_deref() == Some("env"),
            "expected sample env, got {:?}",
            cfg.sample
        );
        ensure!(
            cfg.config_path.is_none(),
            "config_path should not be retained post-merge"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn config_path_cli_overrides_env() -> Result<()> {
    with_jail(|j| {
        j.create_file("env.toml", "sample = \"env\"")?;
        j.create_file("cli.toml", "sample = \"cli\"")?;
        j.set_env("CONFIG_PATH", "env.toml");
        let cfg = RenamedPathConfig::load_from_iter(["prog", "--config", "cli.toml"])
            .map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.sample.as_deref() == Some("cli"),
            "expected sample cli, got {:?}",
            cfg.sample
        );
        ensure!(
            cfg.config_path.is_none(),
            "config_path should not be retained post-merge"
        );
        Ok(())
    })?;
    Ok(())
}
