//! Opt-in build-time export of derived CLI localization identifiers.
//!
//! The pure renderer is intentionally separate from the `OUT_DIR` writer so
//! schema and splitting behaviour stay testable without filesystem state.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use proc_macro2::Span;
use serde::{Deserialize, Serialize};
use syn::Ident;

use super::{LocalizationIds, MessageSuffix};

const CAP_BYTES: usize = 1_048_576;
const SCHEMA_VERSION: u8 = 1;
const ARTEFACT_DIR: &str = "ortho-config";
const FRAGMENT_DIR: &str = "cli-identifiers.d";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Source {
    file: String,
    line: usize,
    column: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Entry {
    id: String,
    kind: String,
    #[serde(rename = "type")]
    type_name: String,
    field: Option<String>,
    path_scope: String,
    source: Source,
    embedded_default: Option<String>,
}

/// Borrows shared schema fields while building entries for one derive expansion.
struct EntryContext<'a> {
    type_name: &'a str,
    source: &'a Source,
}

#[derive(Debug, Deserialize, Serialize)]
struct Document {
    schema_version: u8,
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Index {
    schema_version: u8,
    parts: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Fragment {
    source_file: String,
    entries: Vec<Entry>,
}

#[derive(Debug)]
pub(super) struct ArtefactFile {
    pub(super) name: String,
    pub(super) contents: Vec<u8>,
}

/// Captures the source location associated with one derive expansion.
fn source(span: Span) -> Source {
    let start = span.start();
    Source {
        file: span
            .local_file()
            .map_or_else(|| span.file(), |path| path.to_string_lossy().into_owned()),
        line: start.line,
        column: start.column,
    }
}

/// Builds one schema entry for a command or argument identifier.
fn entry(
    id: impl AsRef<str>,
    kind: MessageSuffix,
    field: Option<String>,
    context: &EntryContext<'_>,
) -> Entry {
    Entry {
        id: id.as_ref().to_owned(),
        kind: kind.as_ref().to_owned(),
        type_name: context.type_name.to_owned(),
        field,
        path_scope: String::from("standalone"),
        source: context.source.clone(),
        embedded_default: None,
    }
}

/// Builds entries for the fixed command-level identifiers in schema order.
fn command_entries(command: &super::CommandIds, context: &EntryContext<'_>) -> Vec<Entry> {
    vec![
        entry(&command.about_id, MessageSuffix::About, None, context),
        entry(
            &command.long_about_id,
            MessageSuffix::LongAbout,
            None,
            context,
        ),
        entry(&command.usage_id, MessageSuffix::Usage, None, context),
        entry(&command.version_id, MessageSuffix::Version, None, context),
        entry(
            &command.long_version_id,
            MessageSuffix::LongVersion,
            None,
            context,
        ),
        entry(
            &command.after_help_id,
            MessageSuffix::AfterHelp,
            None,
            context,
        ),
        entry(
            &command.after_long_help_id,
            MessageSuffix::AfterLongHelp,
            None,
            context,
        ),
    ]
}

/// Builds entries for every argument while preserving model and suffix order.
fn argument_entries(args: &[super::ArgIdsModel], context: &EntryContext<'_>) -> Vec<Entry> {
    let mut output = Vec::with_capacity(args.len() * 3);
    for arg in args {
        let field = Some(arg.field_name.clone());
        output.push(entry(
            &arg.help_id,
            MessageSuffix::Help,
            field.clone(),
            context,
        ));
        output.push(entry(
            &arg.long_help_id,
            MessageSuffix::LongHelp,
            field.clone(),
            context,
        ));
        output.push(entry(
            &arg.value_name_id,
            MessageSuffix::ValueName,
            field,
            context,
        ));
    }
    output
}

/// Converts a localization model into command entries followed by argument entries.
fn entries(model: &LocalizationIds, ident: &Ident, span: Span) -> Vec<Entry> {
    let source = source(span);
    let crate_name = std::env::var("CARGO_CRATE_NAME").unwrap_or_else(|_| String::from("unknown"));
    let type_name = format!("{crate_name}::{ident}");
    let context = EntryContext {
        type_name: &type_name,
        source: &source,
    };
    let mut output = command_entries(&model.command, &context);
    output.extend(argument_entries(&model.args, &context));
    output
}

/// Serializes an artefact value using the stable pretty JSON representation.
fn json<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec_pretty(value)
}

/// Sorts entries so repeated builds produce deterministic output.
fn ordered(mut entries: Vec<Entry>) -> Vec<Entry> {
    entries.sort_by(|left, right| {
        (&left.id, &left.kind, &left.type_name, &left.field).cmp(&(
            &right.id,
            &right.kind,
            &right.type_name,
            &right.field,
        ))
    });
    entries
}

/// Renders one JSON file or a capped set of split files.
fn render(source_entries: Vec<Entry>) -> Result<Vec<ArtefactFile>, serde_json::Error> {
    let ordered_entries = ordered(source_entries);
    let single = json(&Document {
        schema_version: SCHEMA_VERSION,
        entries: ordered_entries.clone(),
    })?;
    if single.len() <= CAP_BYTES {
        return Ok(vec![ArtefactFile {
            name: String::from("cli-identifiers.json"),
            contents: single,
        }]);
    }

    let mut parts = Vec::new();
    let mut batch = Vec::new();
    for entry in ordered_entries {
        batch.push(entry);
        let candidate = json(&Document {
            schema_version: SCHEMA_VERSION,
            entries: batch.clone(),
        })?;
        if candidate.len() > CAP_BYTES && batch.len() > 1 {
            let last = batch.pop();
            parts.push(batch);
            batch = last.into_iter().collect();
        }
    }
    if !batch.is_empty() {
        parts.push(batch);
    }

    let mut output = Vec::new();
    let names = (0..parts.len())
        .map(|index| format!("cli-identifiers.{index}.json"))
        .collect::<Vec<_>>();
    output.push(ArtefactFile {
        name: String::from("cli-identifiers.index.json"),
        contents: json(&Index {
            schema_version: SCHEMA_VERSION,
            parts: names.clone(),
        })?,
    });
    output.extend(
        parts
            .into_iter()
            .zip(names)
            .map(|(part_entries, name)| {
                json(&Document {
                    schema_version: SCHEMA_VERSION,
                    entries: part_entries,
                })
                .map(|contents| ArtefactFile { name, contents })
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(output)
}

/// Returns whether the consuming build explicitly requested artefact output.
fn requested() -> bool {
    std::env::var("ORTHO_CONFIG_EMIT_IDENTIFIERS").as_deref() == Ok("1")
}

/// Replaces a generated file atomically through a sibling temporary file.
fn atomic_write(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, contents)?;
    fs::rename(temporary, path)
}

/// Hashes a source path for a stable per-expansion fragment name.
fn hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// Computes the fragment path for one deriving type and source file.
fn fragment_path(root: &Path, ident: &Ident, source: &Source) -> PathBuf {
    root.join(FRAGMENT_DIR)
        .join(format!("{ident}-{:016x}.json", hash(&source.file)))
}

/// Reads JSON fragments and drops fragments for removed source files.
fn merge_fragments(root: &Path) -> Result<Vec<Entry>, String> {
    let fragments = root.join(FRAGMENT_DIR);
    let mut output = Vec::new();
    for item in fs::read_dir(&fragments).map_err(|error| error.to_string())? {
        let path = item.map_err(|error| error.to_string())?.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| error.to_string())?;
        let fragment: Fragment =
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        if Path::new(&fragment.source_file).exists() {
            output.extend(fragment.entries);
        }
    }
    Ok(output)
}

/// Writes an artefact only when explicitly requested by the consuming build.
pub(super) fn emit(model: &LocalizationIds, ident: &Ident, span: Span) -> syn::Result<()> {
    if !requested() {
        return Ok(());
    }
    let out_dir = std::env::var("OUT_DIR").map_err(|_| syn::Error::new(span, "identifier artefact emission requires OUT_DIR; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or add a build.rs"))?;
    let root = Path::new(&out_dir).join(ARTEFACT_DIR);
    let source = source(span);
    fs::create_dir_all(root.join(FRAGMENT_DIR)).map_err(|error| syn::Error::new(span, format!("cannot create identifier artefact directory {}: {error}; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or fix permissions", root.display())))?;
    let fragment = Fragment {
        source_file: source.file.clone(),
        entries: entries(model, ident, span),
    };
    let path = fragment_path(&root, ident, &source);
    let contents = json(&fragment).map_err(|error| {
        syn::Error::new(
            span,
            format!("cannot serialize identifier artefact: {error}"),
        )
    })?;
    atomic_write(&path, &contents).map_err(|error| syn::Error::new(span, format!("cannot write identifier artefact {}: {error}; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or fix permissions", path.display())))?;
    for file in render(merge_fragments(&root).map_err(|error| syn::Error::new(span, error))?)
        .map_err(|error| syn::Error::new(span, error.to_string()))?
    {
        atomic_write(&root.join(file.name), &file.contents)
            .map_err(|error| syn::Error::new(span, error.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "artefact_tests.rs"]
mod tests;
