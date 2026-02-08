//! Shared text helpers for `PowerShell` artefact rendering.

pub(super) const CRLF: &str = "\r\n";

pub(super) fn push_line(buffer: &mut String, line: &str) {
    buffer.push_str(line);
    buffer.push_str(CRLF);
}

pub(super) fn quote_single(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
