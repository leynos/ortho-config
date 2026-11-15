//! Phase telemetry for the identifier-artefact fixture build.
//!
//! A cold-versus-warm evidence run needs to show, phase by phase, which Cargo
//! units were reused, when the phase ran, and whether the nested Cargo waited
//! for compile admission or the package cache. The sink is therefore an
//! explicit value rather than a lookup inside the build: the ambient
//! environment is read once, at the test's boundary, and the record format and
//! the clock can both be driven directly by a regression.
//!
//! The sink is inert unless `ORTHO_CONFIG_FIXTURE_TELEMETRY` names a file, so
//! an ordinary test run writes nothing.

use super::cargo_messages::BuildCounts;
use anyhow::{Context, Result};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Names the file to which each build phase appends its Cargo unit counts.
pub(super) const TELEMETRY_VARIABLE: &str = "ORTHO_CONFIG_FIXTURE_TELEMETRY";

/// Reads the wall clock as a duration since the Unix epoch.
///
/// Injected so a regression can supply a fixed time and so a failing clock is
/// reported rather than silently recorded as zero: a phase stamped `at_ms=0`
/// would put the whole run's ordering in question without saying so.
pub(super) type Clock = Box<dyn Fn() -> Result<Duration> + Send + Sync>;

/// Where phase records are appended, and what time they carry.
///
/// Reachable at crate visibility because `FixtureLocation::lease` names it in
/// its signature, and that method is reachable from the test crate's root.
pub(crate) struct TelemetrySink {
    directory: Dir,
    file_name: String,
    clock: Clock,
}

impl TelemetrySink {
    /// Build the sink named by the environment, if the variable is set.
    pub(super) fn from_environment() -> Result<Option<Self>> {
        let Some(path) = std::env::var_os(TELEMETRY_VARIABLE) else {
            return Ok(None);
        };
        Self::at_path(Path::new(&path), system_clock()).map(Some)
    }

    /// Append one phase's record, returning the clock error if there was one.
    pub(super) fn record(&self, phase: PhaseRecord<'_>) -> Result<()> {
        let at_ms = (self.clock)()
            .context("read the clock for fixture telemetry")?
            .as_millis();
        let stderr_text = String::from_utf8_lossy(phase.stderr);
        let mut file = self
            .directory
            .open_with(
                &self.file_name,
                OpenOptions::new().create(true).append(true),
            )
            .context("open the fixture telemetry file")?;
        writeln!(
            file,
            "at_ms={at_ms} emit={} {} admission_lines={} package_cache_lines={}",
            phase.emit,
            phase.counts.summary(),
            count_matching_lines(&stderr_text, "[build-limits]"),
            count_matching_lines(&stderr_text, "waiting for file lock"),
        )
        .context("append the fixture build telemetry")
    }

    /// Build a sink for `path`, appending to the file it names.
    ///
    /// The path must name a file; a directory path is refused here rather than
    /// failing later against a name derived from the directory itself.
    pub(super) fn at_path(path: &Path, clock: Clock) -> Result<Self> {
        let file_name = path
            .file_name()
            .with_context(|| {
                format!(
                    "fixture telemetry path {} should name a file",
                    path.display()
                )
            })?
            .to_str()
            .context("fixture telemetry file name should be UTF-8")?
            .to_owned();
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let directory = Dir::open_ambient_dir(parent, ambient_authority())
            .context("open the fixture telemetry directory")?;
        Ok(Self {
            directory,
            file_name,
            clock,
        })
    }
}

/// One build phase's counts and the stderr the build produced.
///
/// Every field is a copy of what the build already holds, so the record is
/// passed by value: there is nothing for the sink to take ownership of.
#[derive(Clone, Copy)]
pub(super) struct PhaseRecord<'a> {
    /// Whether the phase asked the fixture to emit the identifier artefact.
    pub(super) emit: bool,
    /// Fresh and rebuilt unit counts Cargo reported for this build.
    pub(super) counts: BuildCounts,
    /// The build's stderr, scanned for admission and package-cache waits.
    pub(super) stderr: &'a [u8],
}

/// The system clock, reporting a pre-epoch time as an error.
fn system_clock() -> Clock {
    Box::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("read the system clock")
    })
}

/// Count the lines of `text` that contain `needle`.
fn count_matching_lines(text: &str, needle: &str) -> usize {
    text.lines().filter(|line| line.contains(needle)).count()
}

#[cfg(test)]
mod tests {
    //! The record format, the clock boundary and the path boundary.

    use super::super::cargo_messages::BuildCounts;
    use super::{PhaseRecord, TelemetrySink};
    use anyhow::{Context, Result, ensure};
    use cap_std::{ambient_authority, fs::Dir};
    use std::path::Path;
    use std::time::Duration;
    use tempfile::TempDir;

    /// A recorded phase carries the injected time and the tallied waits.
    #[test]
    fn a_recorded_phase_uses_the_injected_clock() -> Result<()> {
        let scratch = TempDir::new().context("create the telemetry directory")?;
        let path = scratch.path().join("phases.log");
        let sink = TelemetrySink::at_path(&path, Box::new(|| Ok(Duration::from_millis(42))))?;
        sink.record(PhaseRecord {
            emit: true,
            counts: BuildCounts::new(3, 1, 1),
            stderr: b"[build-limits] crate: waiting for a compile slot\nother\n",
        })?;
        let recorded = Dir::open_ambient_dir(scratch.path(), ambient_authority())
            .context("open the telemetry directory")?
            .read_to_string("phases.log")
            .context("read the telemetry file")?;
        ensure!(
            recorded
                == "at_ms=42 emit=true fresh=3, rebuilt=1, rebuilt_fixture=1 \
                    admission_lines=1 package_cache_lines=0\n",
            "the phase record is not the documented format:\n{recorded}",
        );
        Ok(())
    }

    /// A clock failure is reported rather than recorded as zero.
    #[test]
    fn a_clock_failure_is_reported() -> Result<()> {
        let scratch = TempDir::new().context("create the telemetry directory")?;
        let sink = TelemetrySink::at_path(
            &scratch.path().join("phases.log"),
            Box::new(|| anyhow::bail!("the clock is unavailable")),
        )?;
        let error = sink
            .record(PhaseRecord {
                emit: false,
                counts: BuildCounts::default(),
                stderr: b"",
            })
            .err()
            .context("a failing clock should not produce a record")?;
        ensure!(
            format!("{error:#}").contains("the clock is unavailable"),
            "the clock error should reach the caller:\n{error:#}",
        );
        Ok(())
    }

    /// A path that names a directory is refused at the boundary.
    #[test]
    fn a_path_without_a_file_name_is_refused() -> Result<()> {
        let error = TelemetrySink::at_path(Path::new("/"), Box::new(|| Ok(Duration::ZERO)))
            .err()
            .context("a directory path should not build a sink")?;
        ensure!(
            format!("{error:#}").contains("should name a file"),
            "the refusal should name the rule it broke:\n{error:#}",
        );
        Ok(())
    }
}
