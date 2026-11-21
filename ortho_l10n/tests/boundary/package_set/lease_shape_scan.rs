//! Reading the boundary sources to count what they call.
//!
//! The shape cases ask questions about source text - "is the packaging
//! boundary entered once", "does `roots.rs` reach the lease" - and answering
//! them needs a matcher that is not fooled by a declaration, an import, a
//! comment, or an argument spelled differently. That matcher and the tree walk
//! built on it are here rather than in the cases themselves, so the cases read
//! as the questions they ask.
//!
//! The split is also what keeps the case module within Whitaker's 400-line
//! limit; it is by responsibility first, size second.

use anyhow::{Context, Result};
use camino::Utf8Path;
use cap_std::fs_utf8::Dir;

/// How many packaging calls `roots.rs` itself makes.
///
/// A single-file read rather than a tree walk, because the question here is
/// about that one module. [`count_in_tree`] opens its root as a directory, so
/// it cannot answer it; the two together are what pin the call to `roots`.
pub(super) fn count_in_roots(sources: &Utf8Path) -> Result<usize> {
    let directory = Dir::open_ambient_dir(
        sources.join("boundary/package_set"),
        cap_std::ambient_authority(),
    )
    .context("open the package_set boundary sources")?;
    let source = directory
        .read_to_string("roots.rs")
        .context("read roots.rs")?;
    Ok(count_calls(&source, "package_sources"))
}

/// How many call expressions to `name` appear in `source`.
///
/// The callee is matched with an optional turbofish and then an opening
/// parenthesis, so the argument spelling cannot hide a call: `(repository`,
/// `( &repository`, a newline before the argument and `::<Budget>(repository`
/// all count. An earlier version pinned the first argument's spelling, so a
/// refactor that borrowed it silently dropped the call from the count while
/// the assertion still passed.
///
/// Declarations, imports and comments are excluded, because they are not
/// calls: a `fn package_sources(` definition, a `use ... package_sources;` and
/// a doc comment naming either would otherwise inflate a tree-wide count.
pub(super) fn count_calls(source: &str, name: &str) -> usize {
    without_comments(source)
        .lines()
        .filter(|line| is_call_line(line, name))
        .count()
}

/// Whether `line` contains a call to `name` rather than a declaration or an
/// import of it.
///
/// Line-granular on purpose: a definition, an import and a call each occupy
/// their own line in this codebase, and a line-level rule is one a reader can
/// check by eye. A turbofish is stripped from the callee before the
/// parenthesis is looked for, so `name::<Budget>(` counts.
fn is_call_line(line: &str, name: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with('*') {
        return false;
    }
    if trimmed.starts_with("use ") || trimmed.split_whitespace().any(|word| word == "fn") {
        return false;
    }
    // `split_once` rather than `find` plus a byte offset: the offset form
    // indexes the string, which can panic on a UTF-8 boundary and is denied by
    // this repository's lint policy.
    let Some((_before, after)) = line.split_once(name) else {
        return false;
    };
    let after_callee = after.strip_prefix("::<").map_or(after, |tail| {
        tail.split_once('>').map_or("", |(_generics, rest)| rest)
    });
    after_callee.starts_with('(')
}

/// `source` with comment lines removed.
fn without_comments(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("//") || trimmed.starts_with('*'))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// How many call expressions to `name` appear in the `.rs` files under `root`.
///
/// Uses [`count_calls`] rather than a substring match, so the tree-wide scan
/// and the single-file scan agree on what a call is. A plain `matches` here
/// counted the needle's own literal in this test and the declaration in
/// `mod.rs`, which made the scan report four packaging sites where there is
/// one.
pub(super) fn count_in_tree(root: &Utf8Path, name: &str) -> Result<usize> {
    let root_directory = Dir::open_ambient_dir(root, cap_std::ambient_authority())
        .with_context(|| format!("open {root}"))?;
    let walk = Walk { root, name };
    let mut total = 0;
    let mut pending = vec![(root_directory, String::new())];
    while let Some((directory, prefix)) = pending.pop() {
        total += walk.count_in_directory(&directory, &prefix, &mut pending)?;
    }
    Ok(total)
}

/// What a tree walk is looking for, and where it started.
///
/// Grouped rather than passed as two more arguments: `root` is only used to
/// name paths in errors and `name` is the callee, so they travel together and
/// keep the per-directory helper within the argument limit.
struct Walk<'a> {
    /// The tree being scanned, for error messages.
    root: &'a Utf8Path,
    /// The callee whose calls are counted.
    name: &'a str,
}

impl Walk<'_> {
    /// Count calls in one directory, queueing its subdirectories on `pending`.
    ///
    /// Split out of [`count_in_tree`] because the walk's per-entry branching -
    /// directory, non-Rust file, Rust file - is three nested conditionals, and
    /// Whitaker's cognitive-complexity rule refuses that many clusters in one
    /// function. Naming the per-entry decision also makes the walk readable:
    /// the loop above is "pop a directory, account for its entries".
    fn count_in_directory(
        &self,
        directory: &Dir,
        prefix: &str,
        pending: &mut Vec<(Dir, String)>,
    ) -> Result<usize> {
        let root = self.root;
        let mut total = 0;
        for listed in directory
            .entries()
            .with_context(|| format!("read {root}/{prefix}"))?
        {
            let entry = listed.with_context(|| format!("read an entry under {root}/{prefix}"))?;
            let entry_name = entry
                .file_name()
                .with_context(|| format!("read a file name under {root}/{prefix}"))?;
            let relative = if prefix.is_empty() {
                entry_name.clone()
            } else {
                format!("{prefix}/{entry_name}")
            };
            let file_type = entry
                .file_type()
                .with_context(|| format!("read the type of {root}/{relative}"))?;
            if file_type.is_dir() {
                pending.push((entry.open_dir().context("open a subdirectory")?, relative));
                continue;
            }
            if !file_type.is_file() || !is_rust_source(&entry_name) {
                continue;
            }
            // Relative to the directory the entry came from, not to the root:
            // `directory` is the parent, so only the final name is a path in it.
            let text = directory
                .read_to_string(&entry_name)
                .with_context(|| format!("read {root}/{relative}"))?;
            total += count_calls(&text, self.name);
        }
        Ok(total)
    }
}

/// Whether `entry_name` names a Rust source file.
fn is_rust_source(entry_name: &str) -> bool {
    Utf8Path::new(entry_name)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
}
