//! Unit tests covering sample configuration helpers within the behavioural harness.
use super::Harness;
use anyhow::{Result, anyhow, ensure};
use rstest::rstest;

#[test]
fn command_result_normalises_windows_newlines() {
    #[cfg(unix)]
    let status = {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    };

    #[cfg(windows)]
    let status = {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    };

    let output = std::process::Output {
        status,
        stdout: b"hello\r\nworld\rnext".to_vec(),
        stderr: b"warn\r\nline".to_vec(),
    };

    let result = super::CommandResult::from_execution(output, "hello_world".into(), vec![]);

    assert!(result.success);
    assert_eq!(result.stdout, "hello\nworld\nnext");
    assert_eq!(result.stderr, "warn\nline");
}

#[rstest]
#[case("nonexistent.toml", "missing")]
#[case("../invalid.toml", "invalid")]
fn try_write_sample_config_reports_expected_errors(
    #[case] sample: &str,
    #[case] expected: &str,
) -> Result<()> {
    let world = Harness::for_tests()?;
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

    let world = Harness::for_tests()?;
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

    let world = Harness::for_tests()?;
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

    let visited_entries: Vec<_> = visited.into_iter().collect();
    ensure!(
        visited_entries == vec!["baseline.toml".to_owned(), "overrides.toml".to_owned()],
        "unexpected visited list: {visited_entries:?}"
    );
    Ok(())
}
