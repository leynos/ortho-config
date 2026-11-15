//! Shared text helpers for `PowerShell` artefact rendering.

/// Canonical line ending used by generated `PowerShell` text artefacts.
pub(super) const CRLF: &str = "\r\n";

/// Append a line and the shared CRLF terminator to a rendered artefact.
pub(super) fn push_line(buffer: &mut String, line: &str) {
    buffer.push_str(line);
    buffer.push_str(CRLF);
}

/// Quote a `PowerShell` single-quoted string, doubling embedded apostrophes.
pub(super) fn quote_single(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
