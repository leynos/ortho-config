//! Catalogue-identifier audit for localized IR trees.
//!
//! Split out of `nested_subcommand_end_to_end` so both the nested test file and
//! its file-size budget stay manageable. The audit reads the fixture's Fluent
//! catalogue and reports only identifier lookups that miss while a differently
//! spelled key exists.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use serde_json::Value;
use std::collections::BTreeSet;
use std::error::Error;
use std::io::Read;

type TestError = Box<dyn Error + Send + Sync>;

/// Asserts that no lookup misses an id the catalogue declares under a
/// different `-`/`_` spelling.
///
/// The runtime preserves `-` and `_` in identifier segments, so a catalogue
/// key whose separator case drifts from the generated id becomes a *silent*
/// miss: the lookup falls back to `[missing: id]` and no test notices, because
/// nothing else reads the fixture catalogue. `clap_derive` keeps the raw Rust
/// field name as the argument id and the metadata suffixes are
/// underscore-separated, so the expected spelling is not derivable from the
/// command line.
///
/// Only the respelling case is reported. An id with no catalogue key at all is
/// deliberate: `usage` has no fixture translation and is expected to fall
/// back, so flagging every miss would demand keys the fixture does not intend
/// to ship.
pub(crate) fn assert_no_respelled_identifier_misses(
    root: &Value,
    package: &str,
) -> Result<(), TestError> {
    let declared = catalogue_identifier_keys("en-US", package)?;
    let mut misses = Vec::new();
    collect_missing_ids(root, &mut misses);

    let respelled = misses
        .into_iter()
        .filter(|id| {
            let kebab = id.replace('_', "-");
            let underscored = id.replace('-', "_");
            [kebab, underscored]
                .iter()
                .any(|variant| variant != id && declared.contains(variant))
        })
        .collect::<Vec<_>>();
    if respelled.is_empty() {
        return Ok(());
    }
    Err(format!(
        "localized IR missed catalogue keys respecified under a different \
         separator spelling: {respelled:?}",
    )
    .into())
}

/// Records the id inside a `[missing: <id>]` placeholder, if `text` holds one.
///
/// The two probes are deliberately sequential rather than chained into a
/// single match: each one slices a different end of the placeholder, and a
/// reader can check them independently.
fn record_missing(text: Option<&str>, misses: &mut Vec<String>) {
    const PLACEHOLDER_PREFIX: &str = "[missing: ";
    let remainder = text.and_then(|candidate| candidate.strip_prefix(PLACEHOLDER_PREFIX));
    if let Some(id) = remainder.and_then(|id| id.strip_suffix(']')) {
        misses.push(id.to_owned());
    }
}

/// Recurses into every child node a command or argument node carries.
///
/// Both child-bearing keys hold arrays of the same node shape, so one flat
/// walk covers them instead of repeating the recursion per key.
fn collect_child_ids(value: &Value, misses: &mut Vec<String>) {
    const CHILD_KEYS: [&str; 2] = ["fields", "subcommands"];
    let children = CHILD_KEYS
        .iter()
        .filter_map(|key| value.get(key).and_then(Value::as_array))
        .flatten();
    for child in children {
        collect_missing_ids(child, misses);
    }
}

/// Collects every `[missing: <id>]` placeholder in a localized IR tree.
///
/// Both the `about`/`synopsis` metadata and each field's `help`/`long_help`
/// carry these placeholders, at every depth of the nested command tree.
///
/// A node that is not a JSON object simply yields no keys; `Value::get`
/// already reports `None` for every key of a non-object, so no shape test is
/// needed before the scan.
fn collect_missing_ids(value: &Value, misses: &mut Vec<String>) {
    const LOCALIZED_KEYS: [&str; 5] = ["about", "synopsis", "help", "long_help", "value_name"];

    for key in LOCALIZED_KEYS {
        record_missing(value.get(key).and_then(Value::as_str), misses);
    }
    collect_child_ids(value, misses);
}

/// Reads the identifier keys declared by a fixture catalogue.
///
/// The lookup keys are Fluent message ids, so the table is the text before the
/// first `=`. Attribute lines (indented continuations) and blank lines are
/// skipped.
fn catalogue_identifier_keys(locale: &str, package: &str) -> Result<BTreeSet<String>, TestError> {
    let workspace_root = super::fixtures::workspace_root()?;
    let relative = Utf8PathBuf::from(format!(
        "tests/fixtures/{package}/locales/{locale}/messages.ftl",
    ));
    let catalogue = read_text(&workspace_root, relative.as_path())?;
    Ok(catalogue
        .lines()
        .filter(|line| !line.starts_with([' ', '\t']))
        .filter_map(|line| line.split_once('='))
        .map(|(key, _)| key.trim().to_owned())
        .filter(|key| !key.is_empty())
        .collect())
}

/// Reads a UTF-8 text file through a capability directory.
///
/// The ambient authority is scoped to the repository root, which this audit
/// only ever reads from.
fn read_text(root: &Utf8PathBuf, relative: &Utf8Path) -> Result<String, TestError> {
    let dir = Dir::open_ambient_dir(root, ambient_authority())?;
    let mut file = dir.open(relative)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}
