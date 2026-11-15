//! XML helpers for MAML rendering.

/// Line terminator used by all generated MAML output for stable file contents.
const CRLF: &str = "\r\n";
/// XML declaration shared by every generated help document.
pub(super) const XML_DECLARATION: &str = r#"<?xml version="1.0" encoding="utf-8"?>"#;
/// Root element with the MAML namespaces required by the emitted command nodes.
pub(super) const HELP_ITEMS_OPEN: &str = concat!(
    r#"<helpItems schema="maml" "#,
    r#"xmlns:maml="http://schemas.microsoft.com/maml/2004/10" "#,
    r#"xmlns:command="http://schemas.microsoft.com/maml/dev/command/2004/10" "#,
    r#"xmlns:dev="http://schemas.microsoft.com/maml/dev/2004/10">"#,
);

/// Format a Rust boolean as the lowercase lexical value expected by XML.
pub(super) const fn bool_attr(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// Escape all five XML-sensitive characters before interpolating text content.
pub(super) fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Small indentation-aware writer that owns the document buffer until finished.
pub(super) struct XmlWriter {
    /// Accumulated XML text; lines use the module's CRLF convention.
    buffer: String,
    /// Current nesting depth, represented as two leading spaces per level.
    indent: usize,
}

impl XmlWriter {
    /// Start an empty document at indentation depth zero.
    pub(super) const fn new() -> Self {
        Self {
            buffer: String::new(),
            indent: 0,
        }
    }

    /// Increase the indentation applied to subsequent lines.
    pub(super) const fn indent(&mut self) {
        self.indent += 1;
    }

    /// Decrease indentation without allowing an unmatched close to underflow.
    pub(super) const fn outdent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    /// Append one line with current indentation and the canonical CRLF ending.
    pub(super) fn line(&mut self, line: &str) {
        for _ in 0..self.indent {
            self.buffer.push_str("  ");
        }
        self.buffer.push_str(line);
        self.buffer.push_str(CRLF);
    }

    /// Consume the writer and return the completed document buffer.
    pub(super) fn finish(self) -> String {
        self.buffer
    }
}
