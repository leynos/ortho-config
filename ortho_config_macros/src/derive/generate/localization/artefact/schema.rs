//! Schema types and the pure renderer for the identifier artefact.
//!
//! Nothing here touches the filesystem or the process environment: the input
//! is a localization model and the output is a set of named byte buffers, so
//! ordering and the split cap can be tested directly. The `OUT_DIR` writer
//! lives in the parent module.

use proc_macro2::Span;
use serde::{Deserialize, Serialize};
use syn::Ident;

use super::super::{LocalizationIds, MessageSuffix};

/// Size above which the inventory splits into an index plus numbered parts.
pub(super) const CAP_BYTES: usize = 1_048_576;

/// Schema revision recorded in every rendered document.
pub(super) const SCHEMA_VERSION: u8 = 1;

/// Records where one derive expansion was written.
///
/// Read back by the merge: a fragment whose source file no longer exists is
/// treated as stale and dropped, so this has to be the path the compiler
/// itself reported rather than a normalized one.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Source {
    /// Source file the expansion came from.
    pub(super) file: String,
    /// One-based line of the deriving type.
    pub(super) line: usize,
    /// Zero-based column of the deriving type.
    pub(super) column: usize,
}

/// Describes one generated Fluent identifier in the rendered inventory.
///
/// This is the artefact's unit of meaning: `id` is the catalogue key a
/// translator resolves, and the remaining fields say which declaration
/// produced it so tooling can map an inventory entry back to the Rust source.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Entry {
    /// Fully normalized Fluent identifier.
    pub(super) id: String,
    /// Identifier category, such as `command` or `argument`.
    pub(super) kind: String,
    /// Rust type the identifier was generated for.
    #[serde(rename = "type")]
    pub(super) type_name: String,
    /// Configuration field that produced an argument id, when there is one.
    pub(super) field: Option<String>,
    /// Whether the identifier belongs to a standalone or mounted command tree.
    pub(super) path_scope: String,
    /// Where the deriving type is declared.
    pub(super) source: Source,
    /// Embedded default text, always `None` while `localized_default` is
    /// deferred.
    pub(super) embedded_default: Option<String>,
}

/// Borrows shared schema fields while building entries for one derive expansion.
pub(super) struct EntryContext<'a> {
    /// Rust type every entry of this expansion was generated for.
    pub(super) type_name: &'a str,
    /// Source location shared by every entry of this expansion.
    pub(super) source: &'a Source,
}

/// Renders one whole inventory as the single-file layout.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Document {
    /// Schema revision, so a reader can reject a document it cannot parse.
    pub(super) schema_version: u8,
    /// Every entry in the inventory, in deterministic order.
    pub(super) entries: Vec<Entry>,
}

/// Lists the numbered parts of a split inventory.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Index {
    /// Schema revision of the parts this index names.
    schema_version: u8,
    /// Filenames of the parts, in the order they were rendered.
    pub(super) parts: Vec<String>,
}

/// Carries one expansion's contribution between write and merge.
///
/// A fragment is deliberately not a `Document`: it records the source file
/// that produced it so the merge can drop contributions whose source has since
/// been deleted, which is what keeps a stale `OUT_DIR` from resurrecting ids.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Fragment {
    /// Source file whose expansion wrote this fragment.
    pub(super) source_file: String,
    /// Entries this expansion contributed.
    pub(super) entries: Vec<Entry>,
}

/// Pairs one rendered filename with its bytes, before publication.
#[derive(Debug)]
pub(super) struct ArtefactFile {
    /// Filename to publish below the artefact root.
    pub(super) name: String,
    /// Serialized contents to write.
    pub(super) contents: Vec<u8>,
}

/// Captures the source location associated with one derive expansion.
pub(super) fn source(span: Span) -> Source {
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
pub(super) fn command_entries(
    command: &super::super::CommandIds,
    context: &EntryContext<'_>,
) -> Vec<Entry> {
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
pub(super) fn argument_entries(
    args: &[super::super::ArgIdsModel],
    context: &EntryContext<'_>,
) -> Vec<Entry> {
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
pub(super) fn entries(
    model: &LocalizationIds,
    ident: &Ident,
    span: Span,
    crate_name: &str,
) -> Vec<Entry> {
    let source = source(span);
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
pub(super) fn json<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec_pretty(value)
}

/// Sorts entries so repeated builds produce deterministic output.
pub(super) fn ordered(mut entries: Vec<Entry>) -> Vec<Entry> {
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
pub(super) fn render(source_entries: Vec<Entry>) -> Result<Vec<ArtefactFile>, serde_json::Error> {
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
