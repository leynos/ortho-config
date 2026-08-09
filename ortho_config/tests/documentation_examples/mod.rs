//! Loads fenced examples from the public README and user's guide.
//!
//! Every fence in either document must be preceded by a stable
//! `tested-example` marker. Tests query the exact published text through this
//! module so copied fixtures cannot drift away from the documentation.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::collections::HashSet;

const DOCUMENT_PATHS: &[&str] = &["README.md", "docs/users-guide.md"];
const MARKER_PREFIX: &str = "<!-- tested-example: ";
const MARKER_SUFFIX: &str = " -->";

#[derive(Clone, Copy)]
struct Cursor {
    source: &'static str,
    line_index: usize,
}

impl Cursor {
    fn error(self, message: &str) -> String {
        format!("{}:{} {message}", self.source, self.line_index + 1)
    }
}

/// One marked fenced example loaded from user-facing documentation.
#[derive(Debug, Eq, PartialEq)]
pub struct DocumentedExample {
    /// Stable identifier declared by the `tested-example` marker.
    pub id: String,
    /// Markdown fence language.
    pub language: String,
    /// Exact text inside the fence, including a trailing newline.
    pub body: String,
    /// Repository-relative source document.
    pub source: &'static str,
    /// One-based line containing the opening fence.
    pub line: usize,
}

/// Load every public example and reject unmarked or duplicate fences.
///
/// # Errors
///
/// Returns an error when a document cannot be read, a marker is malformed, a
/// fence is unmarked or unterminated, or an identifier is duplicated.
pub fn load_documented_examples() -> Result<Vec<DocumentedExample>> {
    let repository = repository_directory()?;
    let mut examples = Vec::new();
    for path in DOCUMENT_PATHS {
        let contents = repository
            .read_to_string(path)
            .with_context(|| format!("read {path}"))?;
        examples.extend(parse_document(path, &contents)?);
    }

    let mut ids = HashSet::new();
    for example in &examples {
        ensure!(
            ids.insert(example.id.as_str()),
            "duplicate tested-example identifier '{}'",
            example.id
        );
    }
    Ok(examples)
}

/// Load the documented example identified by `id`.
///
/// # Errors
///
/// Returns an error when the documents are invalid or `id` is absent.
pub fn documented_example(id: &str) -> Result<DocumentedExample> {
    load_documented_examples()?
        .into_iter()
        .find(|example| example.id == id)
        .with_context(|| format!("documented example '{id}' should exist"))
}

pub(crate) fn parse_document(
    source: &'static str,
    contents: &str,
) -> Result<Vec<DocumentedExample>> {
    let mut lines = contents.lines().enumerate();
    let mut examples = Vec::new();
    let mut ids = HashSet::new();

    while let Some((line_index, line)) = lines.next() {
        let cursor = Cursor { source, line_index };
        if let Some(id) = parse_marker(line) {
            ensure!(
                !id.trim().is_empty(),
                "{}",
                cursor.error("tested-example identifier must not be empty")
            );
            ensure!(ids.insert(id), "duplicate tested-example identifier '{id}'");
            examples.push(read_marked_example(&cursor, id, &mut lines)?);
        } else {
            reject_invalid_example_line(&cursor, line)?;
        }
    }

    Ok(examples)
}

fn repository_directory() -> Result<Dir> {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    Dir::open_ambient_dir(repository_root, ambient_authority()).context("open the repository root")
}

fn reject_invalid_example_line(cursor: &Cursor, line: &str) -> Result<()> {
    ensure!(
        line != format!("{MARKER_PREFIX}{}", MARKER_SUFFIX.trim_start()),
        "{}",
        cursor.error("tested-example identifier must not be empty")
    );
    ensure!(
        !line.starts_with("```"),
        "{}",
        cursor.error("fence is missing a tested-example marker")
    );
    Ok(())
}

fn read_marked_example<'a>(
    cursor: &Cursor,
    id: &str,
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Result<DocumentedExample> {
    let (fence_index, fence) = lines
        .find(|(_, line)| !line.is_empty())
        .with_context(|| cursor.error("marker has no fence"))?;
    let fence_cursor = Cursor {
        source: cursor.source,
        line_index: fence_index,
    };
    let language = fence
        .strip_prefix("```")
        .with_context(|| fence_cursor.error("expected an opening fence after marker"))?;
    ensure!(
        !language.is_empty(),
        "{}",
        fence_cursor.error("fence should declare a language")
    );
    let body = read_fence_body(cursor.source, fence_index, lines)?;
    Ok(DocumentedExample {
        id: id.to_owned(),
        language: language.to_owned(),
        body,
        source: cursor.source,
        line: fence_index + 1,
    })
}

fn parse_marker(line: &str) -> Option<&str> {
    line.strip_prefix(MARKER_PREFIX)
        .and_then(|value| value.strip_suffix(MARKER_SUFFIX))
}

fn read_fence_body<'a>(
    source: &str,
    fence_index: usize,
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Result<String> {
    let mut body = String::new();
    for (_, line) in lines {
        if line == "```" {
            return Ok(body);
        }
        body.push_str(line);
        body.push('\n');
    }
    anyhow::bail!("{source}:{} fence is not terminated", fence_index + 1)
}
