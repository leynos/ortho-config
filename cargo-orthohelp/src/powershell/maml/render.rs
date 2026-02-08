//! Core rendering routines for MAML output.

use crate::ir::{LocalizedFieldMetadata, LocalizedLink};
use crate::schema::ValueType;

use super::types::{CommandSpec, MamlOptions};
use super::xml_writer::{HELP_ITEMS_OPEN, XML_DECLARATION, XmlWriter, bool_attr, escape_xml};

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
        .filter(|field| field.cli.as_ref().is_some_and(|cli| !cli.hide_in_help))
    {
        render_syntax_parameter(writer, field);
    }
    writer.outdent();
    writer.line("</command:syntaxItem>");
    writer.outdent();
    writer.line("</command:syntax>");
}

fn render_syntax_parameter(writer: &mut XmlWriter, field: &LocalizedFieldMetadata) {
    let Some(cli) = field.cli.as_ref() else {
        return;
    };
    let name = parameter_display_name(field);
    let (value_type, is_switch) = parameter_value_type(field);

    writer.line(&format!(
        "<command:parameter required=\"{}\" position=\"named\">",
        bool_attr(field.required)
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
    if cli.multiple {
        writer.line("<command:parameterAttribute variableLength=\"true\" />");
    }
    writer.outdent();
    writer.line("</command:parameter>");
}

fn render_parameters(writer: &mut XmlWriter, command: &CommandSpec<'_>, options: MamlOptions) {
    writer.line("<command:parameters>");
    writer.indent();
    for field in command
        .metadata
        .fields
        .iter()
        .filter(|field| field.cli.as_ref().is_some_and(|cli| !cli.hide_in_help))
    {
        render_parameter_detail(writer, field);
    }
    if options.should_include_common_parameters {
        writer.line("<command:commonParameters />");
    }
    writer.outdent();
    writer.line("</command:parameters>");
}

fn render_parameter_detail(writer: &mut XmlWriter, field: &LocalizedFieldMetadata) {
    let name = parameter_display_name(field);
    let (value_type, is_switch) = parameter_value_type(field);

    writer.line(&format!(
        "<command:parameter required=\"{}\" position=\"named\">",
        bool_attr(field.required)
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

fn render_examples(writer: &mut XmlWriter, command: &CommandSpec<'_>) {
    if command.metadata.sections.examples.is_empty() {
        return;
    }

    writer.line("<command:examples>");
    writer.indent();

    for (index, example) in command.metadata.sections.examples.iter().enumerate() {
        writer.line("<command:example>");
        writer.indent();
        let title = example.title.as_deref().unwrap_or("Example").to_owned();
        writer.line(&format!(
            "<maml:title>{}</maml:title>",
            escape_xml(&format!("{} {}", title, index + 1))
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

const fn parameter_value_type(field: &LocalizedFieldMetadata) -> (&'static str, bool) {
    let Some(cli) = field.cli.as_ref() else {
        return ("String", false);
    };

    if !cli.takes_value {
        return ("SwitchParameter", true);
    }

    match field.value.as_ref() {
        Some(ValueType::Integer { bits, signed }) => {
            if *signed {
                if *bits > 32 {
                    ("Int64", false)
                } else {
                    ("Int32", false)
                }
            } else if *bits > 32 {
                ("UInt64", false)
            } else {
                ("UInt32", false)
            }
        }
        Some(ValueType::Float { bits }) => {
            if *bits > 32 {
                ("Double", false)
            } else {
                ("Single", false)
            }
        }
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

fn parameter_paragraphs(field: &LocalizedFieldMetadata) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let description = field.long_help.as_ref().unwrap_or(&field.help).clone();
    paragraphs.push(description);

    push_cli_paragraphs(field, &mut paragraphs);
    push_default_paragraph(field, &mut paragraphs);
    push_possible_values(field, &mut paragraphs);
    push_source_paragraphs(field, &mut paragraphs);
    push_deprecation_paragraph(field, &mut paragraphs);

    paragraphs
}

fn push_cli_paragraphs(field: &LocalizedFieldMetadata, paragraphs: &mut Vec<String>) {
    let Some(cli) = field.cli.as_ref() else {
        return;
    };
    if let Some(short) = cli.short {
        paragraphs.push(format!("Short flag: -{short}."));
    }
    if let Some(long) = cli.long.as_ref() {
        paragraphs.push(format!("Long flag: --{long}."));
    }
    if cli.multiple {
        paragraphs.push("This option may be supplied multiple times.".to_owned());
    }
}

fn push_default_paragraph(field: &LocalizedFieldMetadata, paragraphs: &mut Vec<String>) {
    let Some(default) = field.default.as_ref() else {
        return;
    };
    paragraphs.push(format!("Default: {}.", default.display));
}

fn push_possible_values(field: &LocalizedFieldMetadata, paragraphs: &mut Vec<String>) {
    let values = collect_possible_values(field);
    if values.is_empty() {
        return;
    }
    paragraphs.push(format!("Possible values: {}.", values.join(", ")));
}

fn collect_possible_values(field: &LocalizedFieldMetadata) -> Vec<String> {
    let mut values = Vec::new();
    if let Some(ValueType::Enum { variants }) = field.value.as_ref() {
        values.extend(variants.iter().cloned());
    }
    if let Some(cli) = field.cli.as_ref() {
        values.extend(cli.possible_values.iter().cloned());
    }
    if values.is_empty() {
        return values;
    }
    values.sort();
    values.dedup();
    values
}

fn push_source_paragraphs(field: &LocalizedFieldMetadata, paragraphs: &mut Vec<String>) {
    if let Some(env) = field.env.as_ref() {
        paragraphs.push(format!("Environment variable: {}.", env.var_name));
    }
    if let Some(file) = field.file.as_ref() {
        paragraphs.push(format!("Config key: {}.", file.key_path));
    }
}

fn push_deprecation_paragraph(field: &LocalizedFieldMetadata, paragraphs: &mut Vec<String>) {
    let Some(deprecation) = field.deprecated.as_ref() else {
        return;
    };
    paragraphs.push(format!("Deprecated: {}.", deprecation.note));
}
