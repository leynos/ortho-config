//! Core rendering routines for MAML output.

use crate::ir::{LocalizedFieldMetadata, LocalizedLink};
use crate::schema::ValueType;
use std::borrow::Cow;

use super::paragraphs::parameter_paragraphs;
use super::types::{CommandSpec, MamlOptions};
use super::xml_writer::{HELP_ITEMS_OPEN, XML_DECLARATION, XmlWriter, bool_attr, escape_xml};

/// Render localized command metadata as a complete MAML help-items document.
///
/// Dynamic values are escaped at their XML text boundary; the writer supplies
/// stable CRLF line endings for the generated help file.
pub(super) fn render_help(commands: &[CommandSpec<'_>], options: MamlOptions) -> String {
    let mut writer = XmlWriter::new();

    writer.line(XML_DECLARATION);
    writer.line(HELP_ITEMS_OPEN);
    writer.indent();
    for command in commands {
        render_command(&mut writer, command, options);
    }
    writer.outdent();
    writer.line("</helpItems>");

    writer.finish()
}

/// Emit one command's details, syntax, parameters, examples and related links.
fn render_command(writer: &mut XmlWriter, command: &CommandSpec<'_>, options: MamlOptions) {
    writer.line("<command:command>");
    writer.indent();

    writer.line("<command:details>");
    writer.indent();
    writer.line(&format!(
        "<command:name>{}</command:name>",
        escape_xml(&command.name)
    ));
    writer.line("<maml:description>");
    writer.indent();
    writer.line(&format!(
        "<maml:para>{}</maml:para>",
        escape_xml(&command.metadata.about)
    ));
    writer.outdent();
    writer.line("</maml:description>");
    writer.outdent();
    writer.line("</command:details>");

    render_syntax(writer, command);
    render_parameters(writer, command, options);
    render_examples(writer, command);
    render_related_links(writer, &command.metadata.sections.links);

    writer.outdent();
    writer.line("</command:command>");
}

/// Emit the command syntax and only those parameters visible in CLI help.
fn render_syntax(writer: &mut XmlWriter, command: &CommandSpec<'_>) {
    writer.line("<command:syntax>");
    writer.indent();
    writer.line("<command:syntaxItem>");
    writer.indent();
    writer.line(&format!(
        "<maml:name>{}</maml:name>",
        escape_xml(&command.name)
    ));
    for field in command
        .metadata
        .fields
        .iter()
        .filter(|field| should_render_parameter(field))
    {
        render_syntax_parameter(writer, field);
    }
    writer.outdent();
    writer.line("</command:syntaxItem>");
    writer.outdent();
    writer.line("</command:syntax>");
}

/// Describe one named CLI parameter in the syntax declaration.
///
/// Fields without CLI metadata are omitted, and switches have no value node.
fn render_syntax_parameter(writer: &mut XmlWriter, field: &LocalizedFieldMetadata) {
    let Some(cli) = field.cli.as_ref() else {
        return;
    };
    let name = parameter_display_name(field);
    let (value_type, is_switch) = parameter_value_type(field);

    writer.line(&format!(
        "<command:parameter required=\"{}\" position=\"named\" variableLength=\"{}\">",
        bool_attr(field.required),
        bool_attr(cli.multiple)
    ));
    writer.indent();
    writer.line(&format!("<maml:name>{}</maml:name>", escape_xml(&name)));
    if !is_switch {
        writer.line(&format!(
            "<command:parameterValue required=\"{}\">{}</command:parameterValue>",
            bool_attr(field.required),
            escape_xml(value_type)
        ));
    }
    writer.outdent();
    writer.line("</command:parameter>");
}

/// Emit parameter details and, when requested, `PowerShell` common parameters.
fn render_parameters(writer: &mut XmlWriter, command: &CommandSpec<'_>, options: MamlOptions) {
    writer.line("<command:parameters>");
    writer.indent();
    for field in command
        .metadata
        .fields
        .iter()
        .filter(|field| should_render_parameter(field))
    {
        render_parameter_detail(writer, field);
    }
    if options.should_include_common_parameters {
        writer.line("<command:commonParameters />");
    }
    writer.outdent();
    writer.line("</command:parameters>");
}

/// Emit a parameter's description and value type using its localized metadata.
fn render_parameter_detail(writer: &mut XmlWriter, field: &LocalizedFieldMetadata) {
    let name = parameter_display_name(field);
    let (value_type, is_switch) = parameter_value_type(field);
    let is_variable_length = field.cli.as_ref().is_some_and(|cli| cli.multiple);

    writer.line(&format!(
        "<command:parameter required=\"{}\" position=\"named\" variableLength=\"{}\">",
        bool_attr(field.required),
        bool_attr(is_variable_length)
    ));
    writer.indent();
    writer.line(&format!("<maml:name>{}</maml:name>", escape_xml(&name)));

    writer.line("<maml:description>");
    writer.indent();
    for paragraph in parameter_paragraphs(field) {
        writer.line(&format!(
            "<maml:para>{}</maml:para>",
            escape_xml(&paragraph)
        ));
    }
    writer.outdent();
    writer.line("</maml:description>");

    if !is_switch {
        writer.line(&format!(
            "<command:parameterValue required=\"{}\">{}</command:parameterValue>",
            bool_attr(field.required),
            escape_xml(value_type)
        ));
    }

    writer.outdent();
    writer.line("</command:parameter>");
}

/// Emit examples in input order, using a numbered title when none was supplied.
fn render_examples(writer: &mut XmlWriter, command: &CommandSpec<'_>) {
    if command.metadata.sections.examples.is_empty() {
        return;
    }

    writer.line("<command:examples>");
    writer.indent();

    for (index, example) in command.metadata.sections.examples.iter().enumerate() {
        writer.line("<command:example>");
        writer.indent();
        let title = example.title.as_deref().map_or_else(
            || Cow::Owned(format!("Example {}", index + 1)),
            Cow::Borrowed,
        );
        writer.line(&format!(
            "<maml:title>{}</maml:title>",
            escape_xml(title.as_ref())
        ));
        writer.line("<maml:code>");
        writer.indent();
        writer.line(&escape_xml(&example.code));
        writer.outdent();
        writer.line("</maml:code>");
        if let Some(body) = example.body.as_ref() {
            writer.line("<maml:remarks>");
            writer.indent();
            writer.line(&format!("<maml:para>{}</maml:para>", escape_xml(body)));
            writer.outdent();
            writer.line("</maml:remarks>");
        }
        writer.outdent();
        writer.line("</command:example>");
    }

    writer.outdent();
    writer.line("</command:examples>");
}

/// Emit related links only when command metadata contains at least one link.
fn render_related_links(writer: &mut XmlWriter, links: &[LocalizedLink]) {
    if links.is_empty() {
        return;
    }

    writer.line("<maml:relatedLinks>");
    writer.indent();
    for link in links {
        writer.line("<maml:navigationLink>");
        writer.indent();
        writer.line(&format!(
            "<maml:linkText>{}</maml:linkText>",
            escape_xml(link.text.as_deref().unwrap_or("Related link"))
        ));
        writer.line(&format!("<maml:uri>{}</maml:uri>", escape_xml(&link.uri)));
        writer.outdent();
        writer.line("</maml:navigationLink>");
    }
    writer.outdent();
    writer.line("</maml:relatedLinks>");
}

/// Decide whether a field belongs in generated `PowerShell` help.
///
/// Hidden CLI fields and fields without CLI metadata are both excluded.
fn should_render_parameter(field: &LocalizedFieldMetadata) -> bool {
    field.cli.as_ref().is_some_and(|cli| !cli.hide_in_help)
}

/// Choose the displayed option spelling, falling back to the configuration key.
fn parameter_display_name(field: &LocalizedFieldMetadata) -> String {
    if let Some(cli) = field.cli.as_ref() {
        if let Some(long) = cli.long.as_ref() {
            return format!("--{long}");
        }
        if let Some(short) = cli.short {
            return format!("-{short}");
        }
    }
    field.name.clone()
}

/// Map an integer width and signedness to the `PowerShell` type name.
const fn integer_type_name(bits: u8, signed: bool) -> &'static str {
    match (signed, bits > 32) {
        (true, true) => "Int64",
        (true, false) => "Int32",
        (false, true) => "UInt64",
        (false, false) => "UInt32",
    }
}

/// Map a float width to the `PowerShell` type name.
const fn float_type_name(bits: u8) -> &'static str {
    if bits > 32 { "Double" } else { "Single" }
}

/// Resolve the `PowerShell` type and switch status used by MAML parameter nodes.
///
/// Missing value metadata deliberately falls back to `String`; non-value CLI
/// flags are represented as `SwitchParameter` without a parameter value node.
#[expect(
    clippy::missing_const_for_fn,
    reason = "runtime-only call sites intentionally keep this helper unconstrained"
)]
fn parameter_value_type(field: &LocalizedFieldMetadata) -> (&'static str, bool) {
    let Some(cli) = field.cli.as_ref() else {
        return ("String", false);
    };

    if !cli.takes_value {
        return ("SwitchParameter", true);
    }

    match field.value.as_ref() {
        Some(ValueType::Integer { bits, signed }) => (integer_type_name(*bits, *signed), false),
        Some(ValueType::Float { bits }) => (float_type_name(*bits), false),
        Some(ValueType::Bool) => ("Boolean", false),
        Some(ValueType::Duration) => ("TimeSpan", false),
        Some(
            ValueType::String
            | ValueType::Path
            | ValueType::IpAddr
            | ValueType::Hostname
            | ValueType::Url
            | ValueType::Enum { .. }
            | ValueType::Custom { .. },
        )
        | None => ("String", false),
        Some(ValueType::List { .. }) => ("String[]", false),
        Some(ValueType::Map { .. }) => ("Hashtable", false),
    }
}
