//! Forward bounded Cargo diagnostics after the build process is reaped.

use std::io::{self, Write};

use super::protocol::CargoMessages;

/// Write retained compiler diagnostics and an explicit truncation notice.
pub(super) fn forward_cargo_diagnostics(messages: &CargoMessages) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    stderr.write_all(messages.diagnostics.as_bytes())?;
    if messages.diagnostics_truncated {
        writeln!(
            stderr,
            "\n[bridge] additional Cargo diagnostics omitted after 1 MiB"
        )?;
    }
    stderr.flush()
}
