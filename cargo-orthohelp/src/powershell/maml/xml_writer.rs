//! XML helpers for MAML rendering.

const CRLF: &str = "\r\n";
pub(super) const XML_DECLARATION: &str = r#"<?xml version="1.0" encoding="utf-8"?>"#;
pub(super) const HELP_ITEMS_OPEN: &str = concat!(
    r#"<helpItems schema="maml" "#,
    r#"xmlns:maml="http://schemas.microsoft.com/maml/2004/10" "#,
    r#"xmlns:command="http://schemas.microsoft.com/maml/dev/command/2004/10" "#,
    r#"xmlns:dev="http://schemas.microsoft.com/maml/dev/2004/10">"#,
);

pub(super) const fn bool_attr(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

pub(super) fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(super) struct XmlWriter {
    buffer: String,
    indent: usize,
}

impl XmlWriter {
    #[expect(
        clippy::missing_const_for_fn,
        reason = "avoid relying on const-stability details for allocation constructors"
    )]
    pub(super) fn new() -> Self {
        Self {
            buffer: String::new(),
            indent: 0,
        }
    }

    pub(super) const fn indent(&mut self) {
        self.indent += 1;
    }

    pub(super) const fn outdent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    pub(super) fn line(&mut self, line: &str) {
        for _ in 0..self.indent {
            self.buffer.push_str("  ");
        }
        self.buffer.push_str(line);
        self.buffer.push_str(CRLF);
    }

    pub(super) fn finish(self) -> String {
        self.buffer
    }
}
