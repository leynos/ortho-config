//! Reading the fixture's Cargo JSON stream while its build runs.
//!
//! The fixture build reports two things the test needs. Its build script names
//! the output directory the identifier artefact is written to, and every
//! compiler unit reports whether Cargo reused it (`fresh`) or recompiled it.
//! The second is the only direct evidence that the stable target keeps the
//! dependency graph warm: a phase that recompiled everything would otherwise
//! look the same as one that reused its dependencies and only rebuilt the
//! fixture package.
//!
//! Records arrive in arbitrary chunks, so a record may be split across two
//! observations. The parser buffers until a newline rather than assuming the
//! chunk boundary matches the record boundary, and bounds the buffer so a
//! stream that never emits a newline cannot grow without limit.

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

/// Upper bound on one buffered Cargo JSON record.
///
/// `cargo metadata` output is small and a `compiler-artifact` record is a few
/// kilobytes even with every path absolute; 128 KiB is far above either and
/// far below a buffer that could exhaust memory.
const MAX_CARGO_RECORD_BYTES: usize = 128 * 1024;

/// Which Cargo units one build reused and which it recompiled.
///
/// Cargo reports every compiler unit in its JSON stream, fresh or not, so these
/// counts distinguish "the stable target kept the dependencies warm" from "the
/// whole dependency graph was rebuilt and the phase merely happened to finish".
#[derive(Clone, Copy, Default)]
pub(crate) struct BuildCounts {
    /// Units Cargo reused from an earlier build.
    fresh: u64,
    /// Units Cargo recompiled, across every package.
    rebuilt: u64,
    /// Recompiled units belonging to `orthohelp_fixture` itself.
    fixture_rebuilt: u64,
}

impl BuildCounts {
    /// Whether only the fixture package was recompiled and dependencies were reused.
    pub(crate) const fn reused_dependencies(self) -> bool {
        self.fresh > 0 && self.fixture_rebuilt > 0 && self.rebuilt == self.fixture_rebuilt
    }

    /// Render the counts for an assertion message.
    pub(crate) fn summary(self) -> String {
        format!(
            "fresh={}, rebuilt={}, rebuilt_fixture={}",
            self.fresh, self.rebuilt, self.fixture_rebuilt,
        )
    }
}

/// The Cargo JSON messages observed during one fixture build.
#[derive(Default)]
pub(super) struct CargoMessages {
    /// The fixture package's identifier, as Cargo reports it.
    package_id: String,
    /// The record currently being assembled.
    line: Vec<u8>,
    /// The fixture build script's output directory, once reported.
    out_dir: Option<PathBuf>,
    /// The first parse error, which stops further parsing.
    error: Option<String>,
    /// Fresh and rebuilt unit counts accumulated from compiler-artifact messages.
    counts: BuildCounts,
}

impl CargoMessages {
    /// Start a parser for `package_id`, the fixture package to track.
    pub(super) fn for_package(package_id: String) -> Self {
        Self {
            package_id,
            ..Self::default()
        }
    }

    /// Feed one chunk of Cargo's JSON stream.
    ///
    /// Once a record has been rejected, later chunks are ignored. Continuing
    /// would parse an unbounded remainder of the stream only to discard it, and
    /// could overwrite the first diagnostic with one from a later record, which
    /// is not the record the reader needs to see.
    pub(super) fn observe(&mut self, bytes: &[u8]) {
        if self.error.is_some() {
            return;
        }
        for byte in bytes {
            if !self.observe_byte(*byte) {
                break;
            }
        }
    }

    /// Consume one byte, reporting whether parsing should continue.
    fn observe_byte(&mut self, byte: u8) -> bool {
        if byte == b'\n' {
            return self.finish_line();
        }
        if self.line.len() == MAX_CARGO_RECORD_BYTES {
            self.error = Some("Cargo JSON record exceeded 128 KiB".to_owned());
            return false;
        }
        self.line.push(byte);
        true
    }

    /// Tally one compiler-artifact message by freshness and owning package.
    fn count_artefact(&mut self, record: &Value) -> Result<()> {
        let fresh = record
            .get("fresh")
            .and_then(Value::as_bool)
            .context("compiler-artifact message omitted `fresh`")?;
        if fresh {
            self.counts.fresh += 1;
            return Ok(());
        }
        self.counts.rebuilt += 1;
        if record.get("package_id").and_then(Value::as_str) == Some(self.package_id.as_str()) {
            self.counts.fixture_rebuilt += 1;
        }
        Ok(())
    }

    /// The counts collected so far.
    pub(super) const fn counts(&self) -> BuildCounts {
        self.counts
    }

    /// Parse the buffered record, stopping on the first error.
    fn finish_line(&mut self) -> bool {
        match self.parse_line() {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }

    /// Finish parsing and return the fixture's build-script directory.
    ///
    /// A rejection already recorded while the stream was being read is
    /// reported first. Parsing a trailing partial record ahead of it would let
    /// a malformed tail replace the diagnostic that explains why the rejected
    /// record was refused, and the first failure is the one that names the
    /// cause.
    pub(super) fn finish(&mut self) -> Result<Option<PathBuf>> {
        if let Some(error) = self.error.take() {
            anyhow::bail!(error);
        }
        if !self.line.is_empty() {
            self.parse_line()?;
        }
        Ok(self.out_dir.take())
    }

    /// Parse one complete record.
    fn parse_line(&mut self) -> Result<()> {
        if self.line.is_empty() {
            return Ok(());
        }
        let record: Value =
            serde_json::from_slice(&self.line).context("parse Cargo JSON record")?;
        self.line.clear();
        let reason = record.get("reason").and_then(Value::as_str);
        if reason == Some("compiler-artifact") {
            self.count_artefact(&record)?;
        }
        if reason == Some("build-script-executed")
            && record.get("package_id").and_then(Value::as_str) == Some(self.package_id.as_str())
        {
            ensure!(
                self.out_dir.is_none(),
                "Cargo reported more than one fixture build-script directory"
            );
            self.out_dir = Some(PathBuf::from(
                record
                    .get("out_dir")
                    .and_then(Value::as_str)
                    .context("fixture build-script message omitted out_dir")?,
            ));
        }
        Ok(())
    }
}

/// Lock the shared parser, tolerating an observer thread that panicked.
pub(super) fn lock_cargo_messages(
    messages: &Mutex<CargoMessages>,
) -> MutexGuard<'_, CargoMessages> {
    messages
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
