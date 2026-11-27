//! SYNOPSIS rendering for roff man pages.
//!
//! The synopsis orders operands by the way clap parses them: ordinary
//! positionals by index, then named flags, then operands that consume the rest
//! of the command line. A `last` operand is reachable only through `--`, so its
//! delimiter is emitted with it and stays optional whenever the operand is.

use crate::ir::{LocalizedFieldMetadata, LocalizedHeadings};
use crate::schema::CliMetadata;

use super::escape::{escape_macro_arg, escape_text, format_option, value_type_placeholder};

/// Generates the SYNOPSIS section content.
pub fn synopsis_section(
    headings: &LocalizedHeadings,
    bin_name: &str,
    synopsis: Option<&str>,
    fields: &[LocalizedFieldMetadata],
) -> String {
    let mut output = format!(
        ".SH {}\n.B {bin_name}\n",
        escape_macro_arg(&headings.synopsis)
    );

    if let Some(syn) = synopsis {
        output.push_str(&escape_text(syn));
        output.push('\n');
    } else {
        push_synopsis_entries(&mut output, fields);
    }

    output
}

/// Emits every visible CLI field in synopsis order.
///
/// Ordinary positional operands lead, ordered by clap's one-based index,
/// because a reader must supply them in that order. Named flags follow in
/// declaration order. An operand that consumes the rest of the command line
/// (`trailing_var_arg`) or is reachable only after `--` (`last`) follows every
/// named flag, because a flag placed after it would be parsed as operand data.
/// A positional renders its value name rather than a flag spelling;
/// [`format_synopsis_option`] already selects that form when the metadata
/// carries neither a long nor a short flag.
fn push_synopsis_entries(output: &mut String, fields: &[LocalizedFieldMetadata]) {
    let visible: Vec<_> = fields
        .iter()
        .filter_map(|field| {
            field
                .cli
                .as_ref()
                .filter(|cli| !cli.hide_in_help)
                .map(|cli| (field, cli))
        })
        .collect();
    let (mut positionals, flags): (Vec<_>, Vec<_>) = visible
        .into_iter()
        .partition(|(_, cli)| cli.positional.is_some());
    positionals.sort_by_key(|(_, cli)| cli.positional.as_ref().map_or(0, |value| value.index));
    let (leading, trailing): (Vec<_>, Vec<_>) = positionals
        .into_iter()
        .partition(|(_, cli)| !is_greedy_operand(cli));

    for (field, cli) in leading {
        output.push_str(&format_synopsis_option(field, cli));
    }
    for (field, cli) in flags {
        output.push_str(&format_synopsis_option(field, cli));
    }
    let (var_args, last_operands): (Vec<_>, Vec<_>) = trailing
        .into_iter()
        .partition(|(_, cli)| !is_last_operand(cli));
    for (field, cli) in var_args {
        output.push_str(&format_synopsis_option(field, cli));
    }
    push_last_operands(output, &last_operands);
}

/// Reports whether an operand consumes the rest of the command line.
fn is_greedy_operand(cli: &CliMetadata) -> bool {
    cli.positional
        .as_ref()
        .is_some_and(|value| value.last || value.var_arg)
}

/// Reports whether an operand is reachable only after the `--` delimiter.
fn is_last_operand(cli: &CliMetadata) -> bool {
    cli.positional.as_ref().is_some_and(|value| value.last)
}

/// Emits the `--` delimiter together with the `last` operands it introduces.
///
/// A required operand makes the delimiter mandatory. When every operand can be
/// omitted, the delimiter is optional too, so it belongs inside the same
/// bracketed group; emitting a bare `--` would mark a delimiter as required
/// that a valid invocation may leave out.
fn push_last_operands(output: &mut String, operands: &[(&LocalizedFieldMetadata, &CliMetadata)]) {
    if operands.is_empty() {
        return;
    }
    if operands.iter().any(|(field, _)| field.required) {
        output.push_str("\\-\\-\n");
        for (field, cli) in operands {
            output.push_str(&format_synopsis_option(field, cli));
        }
    } else {
        let flags: Vec<_> = operands
            .iter()
            .map(|(field, cli)| format_synopsis_flag(field, cli))
            .collect();
        output.push_str("[\\-\\- ");
        output.push_str(&flags.join(" "));
        output.push_str("]\n");
    }
}

/// Formats a synopsis flag without bracketing or a line ending.
///
/// A variadic positional appends `...` so the synopsis shows that the operand
/// accepts repeated values.
fn format_synopsis_flag(field: &LocalizedFieldMetadata, cli: &CliMetadata) -> String {
    let placeholder = field.value.as_ref().map(value_type_placeholder);
    let flag = format_option(cli, placeholder.as_deref(), "VALUE");
    if cli.positional.as_ref().is_some_and(|value| value.variadic) {
        format!("{flag}...")
    } else {
        flag
    }
}

/// Formats a synopsis flag and brackets it unless the corresponding field is required.
fn format_synopsis_option(field: &LocalizedFieldMetadata, cli: &CliMetadata) -> String {
    let flag = format_synopsis_flag(field, cli);

    if field.required {
        format!("{flag}\n")
    } else {
        format!("[{flag}]\n")
    }
}
