#![cfg(test)]
//! Unit tests covering sample configuration helpers within the Cucumber world.
use super::World;
use anyhow::{Result, anyhow, ensure};
use rstest::rstest;

#[rstest]
#[case("nonexistent.toml", "missing")]
#[case("../invalid.toml", "invalid")]
fn try_write_sample_config_reports_expected_errors(
    #[case] sample: &str,
    #[case] expected: &str,
) -> Result<()> {
    let world = World::for_tests()?;
    let Err(error) = world.try_write_sample_config(sample) else {
        return Err(anyhow!("sample config copy should fail"));
    };

    match (expected, error) {
        ("missing", super::SampleConfigError::OpenSample { name, .. })
        | ("invalid", super::SampleConfigError::InvalidName { name }) => {
            ensure!(name == sample, "unexpected sample name: {name}");
        }
        (_, other) => return Err(anyhow!("unexpected sample config error: {other:?}")),
    }
    Ok(())
}

#[rstest]
fn copy_sample_config_writes_all_files() -> Result<()> {
    use anyhow::Context as _;
    use cap_std::fs::Dir;
    use std::collections::BTreeSet;

    let world = World::for_tests()?;
    let tempdir = tempfile::tempdir().context("create sample source")?;
    let source = Dir::open_ambient_dir(tempdir.path(), cap_std::ambient_authority())
        .context("open sample source dir")?;
    source
        .write("overrides.toml", r#"extends = ["baseline.toml"]"#)
        .context("write overrides sample")?;
    source
        .write("baseline.toml", "")
        .context("write baseline sample")?;

    let mut visited = BTreeSet::new();
    let params = super::config::ConfigCopyParams {
        source: &source,
        source_name: "overrides.toml",
        target_name: super::CONFIG_FILE,
    };
    world.copy_sample_config(params, &mut visited)?;

    let scenario = world
        .scenario_dir()
        .context("open hello_world scenario dir")?;
    let overrides = scenario
        .read_to_string(super::CONFIG_FILE)
        .context("read copied overrides")?;
    ensure!(overrides.contains("baseline.toml"));
    let baseline = scenario
        .read_to_string("baseline.toml")
        .context("read copied baseline")?;
    ensure!(baseline.is_empty(), "expected empty baseline");
    Ok(())
}

#[rstest]
fn copy_sample_config_deduplicates_repeated_extends() -> Result<()> {
    use anyhow::Context as _;
    use cap_std::fs::Dir;
    use std::collections::BTreeSet;

    let world = World::for_tests()?;
    let tempdir = tempfile::tempdir().context("create sample source")?;
    let source = Dir::open_ambient_dir(tempdir.path(), cap_std::ambient_authority())
        .context("open sample source dir")?;
    source
        .write(
            "overrides.toml",
            r#"extends = ["baseline.toml", "baseline.toml"]"#,
        )
        .context("write overrides sample")?;
    source
        .write("baseline.toml", "")
        .context("write baseline sample")?;

    let mut visited = BTreeSet::new();
    let params = super::config::ConfigCopyParams {
        source: &source,
        source_name: "overrides.toml",
        target_name: super::CONFIG_FILE,
    };
    world.copy_sample_config(params, &mut visited)?;

    let visited: Vec<_> = visited.into_iter().collect();
    ensure!(
        visited == vec!["baseline.toml".to_owned(), "overrides.toml".to_owned()],
        "unexpected visited list: {:?}",
        visited
    );
    Ok(())
}
