//! Cargo JSON streams for the controlled bridge-build fixtures.
//!
//! Split from `bridge_build_test_support` to keep that file within the
//! repository's 400-line ceiling. Each function stages the files a fake Cargo
//! run would leave behind and returns the JSON stream describing them.

#[cfg(unix)]
use super::set_executable;
use super::{
    FixtureArtifactPaths, FixtureOptions, OVERSIZED_CARGO_RECORD_CHUNK, append_artifact_records,
    binary_file_name, create_directory_all, publish_owner_started, write_bytes,
};
use anyhow::{Context, Result};
use camino::Utf8Path;

/// Produce malformed, oversized, or successful Cargo JSON for a probe.
pub(super) fn fixture_cargo_stream(
    mode: &str,
    options: FixtureOptions<'_>,
    paths: &FixtureArtifactPaths,
) -> Result<Vec<u8>> {
    match mode {
        "malformed" => Ok(b"not-json\n".to_vec()),
        "oversized" => Ok(oversized_cargo_stream()),
        _ => successful_cargo_stream(mode, options, paths),
    }
}

/// A record longer than the reader's line limit, followed by a short one.
fn oversized_cargo_stream() -> Vec<u8> {
    let mut stream = Vec::new();
    for _ in 0..16 {
        stream.extend_from_slice(&OVERSIZED_CARGO_RECORD_CHUNK);
    }
    stream.extend_from_slice(b"x\n");
    stream
}

/// Stage the fake artefacts for a build that reports success, and describe them.
fn successful_cargo_stream(
    mode: &str,
    options: FixtureOptions<'_>,
    paths: &FixtureArtifactPaths,
) -> Result<Vec<u8>> {
    let identity = options.owner.unwrap_or("bridge fixture");
    write_fixture_outputs(mode, identity, paths)?;
    if mode == "owned" {
        publish_owner_started(
            identity,
            options.coordination.context("owner coordination path")?,
        )?;
    }
    let mut stream = Vec::new();
    append_artifact_records(
        &mut stream,
        &paths.executable,
        &[&paths.dependency_output, &paths.dep_info],
        options.binary_name,
    )?;
    if mode == "overlap" {
        write_ordinary_binary(paths)?;
    }
    Ok(stream)
}

/// Write the fake executable, its dependency output and its dep-info file.
fn write_fixture_outputs(mode: &str, identity: &str, paths: &FixtureArtifactPaths) -> Result<()> {
    create_directory_all(artifact_parent(paths)?)?;
    write_bytes(&paths.executable, identity.as_bytes())?;
    if mode == "cleanup-failure" {
        create_directory_all(&paths.dependency_output)?;
    } else {
        write_bytes(&paths.dependency_output, b"generated compiler output")?;
    }
    write_bytes(&paths.dep_info, b"generated dep-info")?;
    #[cfg(unix)]
    set_executable(&paths.executable)?;
    Ok(())
}

/// Write the workspace's ordinary bridge binary beside the fake executable.
fn write_ordinary_binary(paths: &FixtureArtifactPaths) -> Result<()> {
    let ordinary_binary = artifact_parent(paths)?.join(binary_file_name("orthohelp_bridge"));
    write_bytes(&ordinary_binary, b"ordinary workspace binary")
}

/// The directory that holds the fake executable.
fn artifact_parent(paths: &FixtureArtifactPaths) -> Result<&Utf8Path> {
    paths
        .executable
        .parent()
        .context("finding fake artifact parent")
}
