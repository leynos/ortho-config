//! English rendering of the missing-required-values report.
//!
//! Rendering is a pure projection of the typed payload: it reads no arguments,
//! environment, or filesystem, and it never consults a localizer. The report
//! groups every advertised route by provider, emitting one numbered section per
//! provider that has at least one route, and it withholds the route prompt
//! entirely when no route could be rendered.
//!
//! Key spelling lives here too. `ConfigPath` carries segments only, and the
//! TOML spelling of a segment — bare where TOML allows it, quoted and escaped
//! otherwise — is decided by one routine that both the configuration-file
//! section and [`ConfigPath::render`] call, so one message cannot spell a key
//! two ways.

use std::fmt::{self, Write as _};

use super::{ConfigPath, MissingRequiredValue, MissingRequiredValues, SupplyRoute};

impl ConfigPath {
    /// Render the path using dots, quoting segments that need it.
    ///
    /// A segment is quoted whenever TOML would not read it as a bare key. The
    /// quoted form keeps a literal dotted key visibly distinct from a nested
    /// path with the same spelling, and it is the only spelling that names a
    /// key outside TOML's bare-key alphabet — such as one containing `=`.
    #[must_use]
    pub fn render(&self) -> String {
        self.segments()
            .iter()
            .map(|segment| render_segment(segment))
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl fmt::Display for ConfigPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render())
    }
}

/// Render one path segment the way a TOML document accepts it.
///
/// A bare key may contain only ASCII letters, digits, `-` and `_`. Anything
/// else has to be a quoted key, and a quoted key is a TOML basic string, so the
/// quote, the backslash, and every control character are written as escapes
/// rather than emitted raw. Both the rendered report and the suggested file
/// section spell keys through this one routine, so a character that needs an
/// escape cannot be spelled two different ways in one message.
fn render_segment(segment: &str) -> String {
    if is_bare_key(segment) {
        return segment.to_owned();
    }
    let mut quoted = String::with_capacity(segment.len() + 2);
    quoted.push('"');
    for character in segment.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\u{8}' => quoted.push_str("\\b"),
            '\t' => quoted.push_str("\\t"),
            '\n' => quoted.push_str("\\n"),
            '\u{c}' => quoted.push_str("\\f"),
            '\r' => quoted.push_str("\\r"),
            // Writing into a `String` cannot fail; `fmt::Write` reports a
            // result only so the same call can target a fallible sink.
            other if other.is_control() => _ = write!(quoted, "\\u{:04X}", u32::from(other)),
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

/// Report whether TOML reads `segment` as a key without quotes.
///
/// An empty segment, a dot, whitespace, and any character outside
/// `A-Za-z0-9_-` all require the quoted form, so this one predicate replaces
/// the case-by-case list of characters that happened to be considered.
fn is_bare_key(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

impl fmt::Display for MissingRequiredValues {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_heading(formatter, self.program())?;
        write_missing_list(formatter, self.items())?;
        write_route_sections(formatter, self.items())
    }
}

/// Write the report heading and the first prompt line.
fn write_heading(formatter: &mut fmt::Formatter<'_>, program: Option<&str>) -> fmt::Result {
    match program {
        Some(label) => writeln!(formatter, "Missing required configuration for `{label}`")?,
        None => writeln!(formatter, "Missing required configuration")?,
    }
    writeln!(formatter)?;
    writeln!(formatter, "The following value(s) are missing:")
}

/// Write the ordered list of missing canonical paths.
fn write_missing_list(
    formatter: &mut fmt::Formatter<'_>,
    missing: &[MissingRequiredValue],
) -> fmt::Result {
    for item in missing {
        writeln!(formatter, "  - `{}`", item.path)?;
    }
    Ok(())
}

/// Write the grouped supply-route sections.
fn write_route_sections(
    formatter: &mut fmt::Formatter<'_>,
    missing: &[MissingRequiredValue],
) -> fmt::Result {
    let cli = collect_routes(missing, |route| match route {
        SupplyRoute::CommandLine {
            flag,
            takes_value: true,
        } => Some(format!("{flag} <value>")),
        SupplyRoute::CommandLine {
            flag,
            takes_value: false,
        } => Some(flag.clone()),
        _ => None,
    });
    let environment = collect_routes(missing, |route| match route {
        SupplyRoute::Environment { variable } => Some(format!("{variable}=<value>")),
        _ => None,
    });
    let file = file_section(missing).render();

    if has_no_route_lines(&cli, &environment, &file) {
        return Ok(());
    }
    writeln!(formatter)?;
    writeln!(
        formatter,
        "Please provide them using one of the following methods, in order of precedence:"
    )?;

    let sections = [
        ("Command-Line Argument", &cli),
        ("Environment Variable", &environment),
        ("Configuration File", &file),
    ]
    .into_iter()
    .filter(|(_, lines)| !lines.is_empty());
    for (index, (title, lines)) in sections.enumerate() {
        writeln!(formatter, "  {}. {title}:", index + 1)?;
        write_indented_lines(formatter, lines)?;
    }
    Ok(())
}

/// Report whether no route list produced a rendered line.
///
/// A report with no renderable route would otherwise print a section prompt
/// followed by nothing, so the prompt is withheld entirely.
const fn has_no_route_lines(cli: &[String], environment: &[String], file: &[String]) -> bool {
    cli.is_empty() && environment.is_empty() && file.is_empty()
}

/// Collect one rendered line per route selected by `render`.
fn collect_routes<F>(missing: &[MissingRequiredValue], render: F) -> Vec<String>
where
    F: Fn(&SupplyRoute) -> Option<String>,
{
    missing
        .iter()
        .flat_map(|item| item.routes.iter())
        .filter_map(render)
        .collect()
}

/// Write rendered route lines at the report's continuation indent.
fn write_indented_lines(formatter: &mut fmt::Formatter<'_>, lines: &[String]) -> fmt::Result {
    for line in lines {
        for part in line.lines() {
            writeln!(formatter, "     {part}")?;
        }
    }
    Ok(())
}

/// Collect every configuration-file route into one valid TOML document.
///
/// Routes arrive in report order, which may interleave root keys with table
/// keys in any order. TOML assigns each bare key to the most recently opened
/// table and rejects a repeated header, so the section is regrouped here: root
/// assignments first, then one header per table in first-appearance order.
/// Every key is rendered through [`render_segment`], because a bare key accepts
/// only ASCII letters, digits, `-` and `_` — in assignments as well as headers.
#[derive(Default)]
struct FileSection {
    /// Assignments that belong to the document root.
    root: Vec<String>,
    /// Table headers with their assignments, in first-appearance order.
    tables: Vec<(Vec<String>, Vec<String>)>,
}

impl FileSection {
    /// Add one configuration-file route.
    fn push(&mut self, path: &ConfigPath, expects_object: bool) {
        if expects_object {
            if !path.is_empty() {
                self.open_table(path.segments().to_vec());
            }
            return;
        }
        let Some((last, parent)) = path.segments().split_last() else {
            return;
        };
        let assignment = format!("{} = \"<value>\"", render_segment(last));
        if parent.is_empty() {
            self.root.push(assignment);
        } else {
            self.push_assignment(parent.to_vec(), assignment);
        }
    }

    /// Open a table that may receive no assignment, so its header still prints.
    fn open_table(&mut self, header: Vec<String>) {
        if !self.has_table(&header) {
            self.tables.push((header, Vec::new()));
        }
    }

    /// Append one assignment to `header`, opening the table on first use.
    fn push_assignment(&mut self, header: Vec<String>, assignment: String) {
        if let Some((_, assignments)) = self.tables.iter_mut().find(|(known, _)| *known == header) {
            assignments.push(assignment);
        } else {
            self.tables.push((header, vec![assignment]));
        }
    }

    /// Report whether `header` has already been opened.
    fn has_table(&self, header: &[String]) -> bool {
        self.tables.iter().any(|(known, _)| known == header)
    }

    /// Render one physical line per header or assignment.
    fn render(&self) -> Vec<String> {
        let mut lines = self.root.clone();
        for (header, assignments) in &self.tables {
            let rendered: Vec<String> = header
                .iter()
                .map(|segment| render_segment(segment))
                .collect();
            lines.push(format!("[{}]", rendered.join(".")));
            lines.extend(assignments.iter().cloned());
        }
        lines
    }
}

/// Build the configuration-file section from every file route in `missing`.
fn file_section(missing: &[MissingRequiredValue]) -> FileSection {
    let mut section = FileSection::default();
    for item in missing {
        for route in &item.routes {
            if let SupplyRoute::ConfigurationFile {
                path,
                expects_object,
            } = route
            {
                section.push(path, *expects_object);
            }
        }
    }
    section
}
