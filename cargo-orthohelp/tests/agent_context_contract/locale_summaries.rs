//! Locale-scoped agent-context summary contracts.
//!
//! These tests live in a submodule so the parent integration target stays
//! within the repository's file-length limit; they still exercise the same
//! compiled binary and shared stream assertions.

use super::*;

/// Root type whose command tree has both a root and a nested subcommand.
const NESTED_ROOT_TYPE: &str = "orthohelp_fixture::NestedFixtureConfig";

/// A requested locale with a catalogue supplies root and nested summaries.
#[rstest]
fn requested_locale_supplies_agent_context_summaries() -> TestResult {
    let out_dir = tempfile::tempdir()?;
    let output = run_agent_context_with(
        &out_dir,
        Some(NESTED_ROOT_TYPE),
        &["--format", "agent-context", "--locale", "fr-FR"],
        "debug",
    )?;
    ensure_success(&output)?;

    let context = read_stdout_agent_context(&output)?;
    assert_string_field(
        root_command(&context)?,
        "summary",
        "Arborescence de commandes du fixture imbriqué.",
    )?;
    assert_string_field(
        command_by_path(&context, &["nested_fixture", "greet"])
            .ok_or("the greet subcommand should exist")?,
        "summary",
        "Salue un destinataire nommé.",
    )?;
    // stdout is compact while the artefact is pretty-printed, so equality is
    // semantic: both writers must serialise the same document.
    if context != read_agent_context(&out_dir)? {
        return Err("localized agent-context stdout should match its artefact".into());
    }
    Ok(())
}

/// `--format all` localizes the IR from the same catalogue as the summaries.
#[rstest]
fn all_format_reuses_the_summary_catalogue_for_localized_ir() -> TestResult {
    let out_dir = tempfile::tempdir()?;
    let output = run_agent_context_with(
        &out_dir,
        Some(NESTED_ROOT_TYPE),
        &["--format", "all", "--locale", "fr-FR"],
        "debug",
    )?;
    ensure_success(&output)?;

    let context = read_stdout_agent_context(&output)?;
    let ir = read_localized_ir(&out_dir, "fr-FR")?;
    assert_string_field(&ir, "locale", "fr-FR")?;
    assert_string_field(&ir, "about", summary_for(&context, &["nested_fixture"])?)?;
    let greet = subcommand_by_name(&ir, "greet")
        .ok_or("the fr-FR IR should contain the greet subcommand")?;
    assert_string_field(
        greet,
        "about",
        summary_for(&context, &["nested_fixture", "greet"])?,
    )?;
    Ok(())
}

/// The chosen locale and its source are visible on stderr, not on stdout.
#[rstest]
fn info_tracing_reports_the_summary_locale_choice() -> TestResult {
    let out_dir = tempfile::tempdir()?;
    let output = run_agent_context_with(
        &out_dir,
        None,
        &["--format", "agent-context", "--locale", "fr-FR"],
        "info",
    )?;
    ensure_success(&output)?;
    assert_one_compact_json_line(&output.stdout)?;

    // The subscriber styles fields with ANSI escapes even when stderr is
    // captured, so strip them before asserting on the rendered field pairs.
    let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
    let event = stderr
        .lines()
        .find(|line| line.contains("agent-context summary locale selected"))
        .ok_or_else(|| format!("stderr should report the selected locale, got {stderr}"))?;
    for expected in ["locale=fr-FR", "source=\"requested\""] {
        if !event.contains(expected) {
            return Err(format!("event should report {expected:?}, got {event}").into());
        }
    }
    Ok(())
}

/// Removes ANSI escape sequences from a captured stream.
fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut characters = input.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            output.push(character);
            continue;
        }
        if characters.peek() == Some(&'[') {
            characters.next();
            skip_control_sequence(&mut characters);
        }
    }
    output
}

/// Consumes one CSI sequence up to and including its final ASCII letter.
fn skip_control_sequence(characters: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    for escape in characters {
        if escape.is_ascii_alphabetic() {
            break;
        }
    }
}

/// Borrows the first command in an agent-context document.
///
/// # Errors
///
/// Returns an error when the document has no `commands` array or it is empty.
fn root_command(context: &Value) -> TestResult<&Value> {
    context
        .get("commands")
        .and_then(Value::as_array)
        .and_then(|commands| commands.first())
        .ok_or_else(|| "agent context should contain a root command".into())
}

/// Finds the command whose `path` equals `path` exactly.
///
/// Comparison is element-wise over the whole path, so a subcommand is not
/// matched by a shorter prefix or a shared leaf name. Returns `None` when the
/// document has no `commands` array or no command matches.
fn command_by_path<'a>(context: &'a Value, path: &[&str]) -> Option<&'a Value> {
    context
        .get("commands")
        .and_then(Value::as_array)?
        .iter()
        .find(|command| {
            command
                .get("path")
                .and_then(Value::as_array)
                .is_some_and(|segments| {
                    segments.len() == path.len()
                        && segments
                            .iter()
                            .zip(path)
                            .all(|(segment, expected)| segment.as_str() == Some(*expected))
                })
        })
}

/// Reads the summary of the command identified by `path`.
///
/// # Errors
///
/// Returns an error when no command matches `path`, or when its `summary`
/// field is absent or not a string.
fn summary_for<'a>(context: &'a Value, path: &[&str]) -> TestResult<&'a str> {
    let command =
        command_by_path(context, path).ok_or_else(|| format!("command path {path:?} missing"))?;
    string_field(command, "summary")
}

/// Reads and parses `<out>/ir/<locale>.json`.
///
/// # Errors
///
/// Returns an error when the output path is not UTF-8, the artefact is absent
/// or unreadable, or its contents are not valid JSON.
fn read_localized_ir(out_dir: &TempDir, locale: &str) -> TestResult<Value> {
    let out_path = Utf8PathBuf::from_path_buf(out_dir.path().to_path_buf())
        .map_err(|path| format!("non-UTF-8 output path: {}", path.display()))?;
    let directory = Dir::open_ambient_dir(out_path, ambient_authority())?;
    let json = directory.read_to_string(format!("ir/{locale}.json"))?;
    Ok(serde_json::from_str(&json)?)
}

/// Finds a direct subcommand of a localized IR document by `app_name`.
///
/// Only the document's immediate children are searched. Returns `None` when
/// there are no subcommands or none carry that `app_name`.
fn subcommand_by_name<'a>(ir: &'a Value, name: &str) -> Option<&'a Value> {
    ir.get("subcommands")
        .and_then(Value::as_array)?
        .iter()
        .find(|command| command.get("app_name").and_then(Value::as_str) == Some(name))
}
