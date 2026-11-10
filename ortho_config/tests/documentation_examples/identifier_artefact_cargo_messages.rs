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
pub(super) const MAX_CARGO_RECORD_BYTES: usize = 128 * 1024;

/// Which Cargo units one build reused and which it recompiled.
///
/// Cargo reports every compiler unit in its JSON stream, fresh or not, so these
/// counts distinguish "the stable target kept the dependencies warm" from "the
/// whole dependency graph was rebuilt and the phase merely happened to finish".
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct BuildCounts {
    /// Units Cargo reused from an earlier build.
    fresh: u64,
    /// Units Cargo recompiled, across every package.
    rebuilt: u64,
    /// Recompiled units belonging to `orthohelp_fixture` itself.
    fixture_rebuilt: u64,
}

impl BuildCounts {
    /// State a count triple directly, for tests that name the expected numbers.
    ///
    /// Production code only ever obtains counts from Cargo's stream; the
    /// constructor exists so a regression can exercise the decision without
    /// driving a real build.
    #[cfg(test)]
    pub(super) const fn new(fresh: u64, rebuilt: u64, fixture_rebuilt: u64) -> Self {
        Self {
            fresh,
            rebuilt,
            fixture_rebuilt,
        }
    }

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

#[cfg(test)]
mod tests {
    //! The reuse decision, the record boundaries and the parser's refusals.

    use super::{BuildCounts, CargoMessages, MAX_CARGO_RECORD_BYTES};
    use anyhow::{Context, Result, ensure};
    use rstest::rstest;
    use serde_json::json;
    use std::path::Path;

    /// The fixture package, as Cargo prints it in `package_id`.
    const FIXTURE: &str = "path+file:///fixture#orthohelp_fixture@0.1.0";

    /// Reuse holds only when fresh units exist and nothing but the fixture was
    /// rebuilt.
    ///
    /// Each case removes one of the three conditions while keeping the other
    /// two, so a decision that dropped any one of them would pass none of the
    /// failing cases by accident.
    #[rstest]
    #[case::dependencies_reused(5, 1, 1, true)]
    #[case::everything_rebuilt(0, 4, 1, false)]
    #[case::nothing_rebuilt(5, 0, 0, false)]
    #[case::another_package_rebuilt(5, 2, 1, false)]
    #[case::no_units_at_all(0, 0, 0, false)]
    fn reuse_needs_fresh_units_and_only_the_fixture_rebuilt(
        #[case] fresh: u64,
        #[case] rebuilt: u64,
        #[case] fixture_rebuilt: u64,
        #[case] expected: bool,
    ) -> Result<()> {
        let counts = BuildCounts::new(fresh, rebuilt, fixture_rebuilt);
        ensure!(
            counts.reused_dependencies() == expected,
            "{} should {} report reuse",
            counts.summary(),
            if expected { "" } else { "not" },
        );
        Ok(())
    }

    /// Freshness and the owning package decide which counter a unit reaches.
    #[test]
    fn compiler_artefacts_are_counted_by_freshness_and_package() -> Result<()> {
        let mut messages = CargoMessages::for_package(FIXTURE.to_owned());
        for record in [
            json!({"reason": "compiler-artifact", "fresh": true, "package_id": FIXTURE}),
            json!({
                "reason": "compiler-artifact",
                "fresh": false,
                "package_id": "path+file:///other#serde@1.0.0",
            }),
            json!({"reason": "compiler-artifact", "fresh": false, "package_id": FIXTURE}),
        ] {
            messages.observe(format!("{record}\n").as_bytes());
        }
        messages.finish()?;
        ensure!(
            messages.counts() == BuildCounts::new(1, 2, 1),
            "the counts should be {}:\n{}",
            BuildCounts::new(1, 2, 1).summary(),
            messages.counts().summary(),
        );
        Ok(())
    }

    /// A record split across observations is reassembled before parsing.
    #[test]
    fn a_record_split_across_chunks_is_reassembled() -> Result<()> {
        let out_dir = "/tmp/fixture-out";
        let record = format!(
            "{{\"reason\":\"build-script-executed\",\
             \"package_id\":\"{FIXTURE}\",\"out_dir\":\"{out_dir}\"}}\n"
        );
        let mut messages = CargoMessages::for_package(FIXTURE.to_owned());
        for chunk in record.as_bytes().chunks(7) {
            messages.observe(chunk);
        }
        let reported = messages
            .finish()?
            .context("the split record should report the build-script directory")?;
        ensure!(
            reported == Path::new(out_dir),
            "the split record reported {} instead of {out_dir}",
            reported.display(),
        );
        Ok(())
    }

    /// A record without a newline is still parsed when the stream ends.
    #[test]
    fn a_trailing_record_without_a_newline_is_parsed_at_finish() -> Result<()> {
        let mut messages = CargoMessages::for_package(FIXTURE.to_owned());
        messages.observe(b"{\"reason\":\"compiler-artifact\",\"fresh\":true,\"package_id\":\"x\"}");
        messages.finish()?;
        ensure!(
            messages.counts() == BuildCounts::new(1, 0, 0),
            "the trailing record should have been counted:\n{}",
            messages.counts().summary(),
        );
        Ok(())
    }

    /// A compiler-artifact record that omits `fresh`.
    const WITHOUT_FRESH: &[u8] = b"{\"reason\":\"compiler-artifact\",\"package_id\":\"x\"}\n";

    /// Feed `records` to a fresh observer and render the rejection it reports.
    ///
    /// The rejection regressions differ in what they observe and in the message
    /// they require, but not in how they reach the error, so that path lives
    /// here once.
    fn rejection_of(records: &[&[u8]]) -> Result<String> {
        let mut messages = CargoMessages::for_package(FIXTURE.to_owned());
        for record in records {
            messages.observe(record);
        }
        let error = messages
            .finish()
            .err()
            .context("the stream should be rejected")?;
        Ok(format!("{error:#}"))
    }

    /// A record without `fresh` is a hard error, and a malformed record that
    /// follows it does not displace that rejection.
    #[rstest]
    #[case::the_missing_field(&[WITHOUT_FRESH])]
    #[case::a_malformed_tail(&[WITHOUT_FRESH, b"{not json}"])]
    fn a_record_without_a_fresh_flag_is_rejected(#[case] records: &[&[u8]]) -> Result<()> {
        let error = rejection_of(records)?;
        ensure!(
            error.contains("`fresh`"),
            "the rejection should name the missing field:\n{error}",
        );
        Ok(())
    }

    /// An over-long record is refused, and the first refusal is the one kept.
    #[test]
    fn an_over_long_record_stops_the_parser_and_keeps_its_error() -> Result<()> {
        let long = vec![b' '; MAX_CARGO_RECORD_BYTES + 1];
        // A later malformed record must not replace the diagnostic that
        // explains why parsing stopped.
        let error = rejection_of(&[&long, b"{not json}\n"])?;
        ensure!(
            error.contains("128 KiB"),
            "the rejection should state the bound:\n{error}",
        );
        Ok(())
    }

    /// Two build-script directories for the fixture are a hard error.
    #[test]
    fn a_second_build_script_directory_is_rejected() -> Result<()> {
        let mut messages = CargoMessages::for_package(FIXTURE.to_owned());
        for out_dir in ["/tmp/one", "/tmp/two"] {
            let record = json!({
                "reason": "build-script-executed",
                "package_id": FIXTURE,
                "out_dir": out_dir,
            });
            messages.observe(format!("{record}\n").as_bytes());
        }
        let error = messages
            .finish()
            .err()
            .context("a duplicate build-script directory should be rejected")?;
        ensure!(
            format!("{error:#}").contains("more than one"),
            "the rejection should explain the duplicate:\n{error:#}",
        );
        Ok(())
    }
}
