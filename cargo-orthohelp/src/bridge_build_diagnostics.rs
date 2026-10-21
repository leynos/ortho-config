//! Forward bounded Cargo diagnostics after the build process is reaped.

use std::io::{self, Write};

use super::protocol::CargoMessages;

/// Notice appended once retained diagnostics were bounded rather than complete.
const TRUNCATION_NOTICE: &str = "\n[bridge] additional Cargo diagnostics omitted after 1 MiB";

/// Write retained compiler diagnostics and an explicit truncation notice.
pub(super) fn forward_cargo_diagnostics(messages: &CargoMessages) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    write_cargo_diagnostics(&mut stderr, messages)
}

/// Emit the retained diagnostics to an explicit sink, then flush it.
///
/// The sink parameter keeps the forwarding contract testable: a no-op
/// implementation would satisfy any assertion made against the parser alone.
fn write_cargo_diagnostics(sink: &mut impl Write, messages: &CargoMessages) -> io::Result<()> {
    sink.write_all(messages.diagnostics.as_bytes())?;
    if messages.diagnostics_truncated {
        writeln!(sink, "{TRUNCATION_NOTICE}")?;
    }
    sink.flush()
}

#[cfg(test)]
#[path = "bridge_build_diagnostics_tests.rs"]
mod tests;
