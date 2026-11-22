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

/// The name of a function whose calls are counted.
///
/// A newtype rather than a bare `&str` because it is a domain value - the
/// callee - and not arbitrary text: every function below takes one, and a
/// caller cannot pass a file name or a needle by mistake. It also keeps the
/// module's string-argument ratio under `CodeScene`'s threshold, which is a
/// symptom of the same thing rather than a separate goal.
#[derive(Clone, Copy)]
pub(super) struct Callee<'a>(&'a str);

impl<'a> Callee<'a> {
    /// Name the callee whose calls are counted.
    pub(super) const fn named(name: &'a str) -> Self {
        Self(name)
    }

    /// The callee's name as text, for matching.
    const fn as_str(self) -> &'a str {
        self.0
    }
}

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
    // Assembled rather than literal: the tree-wide scan reads this file, and a
    // bare callee literal here would be counted as a packaging call site - the
    // scanner reporting itself. The shape cases pin the count, so this must not
    // create a site it would report.
    let packaging = ["package", "_sources"].concat();
    Ok(count_calls(&source, Callee::named(&packaging)))
}

/// How many call expressions to `name` appear in `source`.
///
/// The callee is matched with an optional turbofish, optional whitespace, and
/// then an opening parenthesis, over the whole source rather than line by
/// line, so no spelling of the gap between callee and arguments can hide a
/// call: `(repository`, `( &repository`, a newline before or after the
/// parenthesis, and `::<Budget>(repository` all count. An earlier version
/// pinned the first argument's spelling, and a later one tested a single line,
/// so a refactor or a line break silently dropped the call from the count
/// while the assertion still passed.
///
/// Declarations, imports and comments are excluded, because they are not
/// calls: a `fn package_sources(` definition, a `use ... package_sources;` and
/// a doc comment naming either would otherwise inflate a tree-wide count.
pub(super) fn count_calls(source: &str, name: Callee<'_>) -> usize {
    let code = without_comments(source);
    let mut remaining = code.as_str();
    let mut total = 0;
    while let Some((before, after)) = remaining.split_once(name.as_str()) {
        if is_call_position(before, after) {
            total += 1;
        }
        remaining = after;
    }
    total
}

/// Whether a callee occurrence at `before`/`after` is a call rather than a
/// declaration or an import.
///
/// `before` is the text preceding the callee on its own line, so the
/// declaration and import tests stay line-scoped while the parenthesis test
/// spans lines.
fn is_call_position(before: &str, after: &str) -> bool {
    // `rsplit_once` rather than `rfind` plus an offset: indexing can panic on
    // a UTF-8 boundary and is denied by this repository's lint policy.
    let line = before
        .rsplit_once('\n')
        .map_or(before, |(_prev, line)| line);
    if line.trim_start().starts_with("use ") {
        return false;
    }
    if before
        .split_whitespace()
        .rev()
        .take(4)
        .any(|word| word == "fn")
    {
        return false;
    }
    let after_callee = after.strip_prefix("::<").map_or(after, |tail| {
        tail.split_once('>').map_or("", |(_generics, rest)| rest)
    });
    after_callee.trim_start().starts_with('(')
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
pub(super) fn count_in_tree(root: &Utf8Path, name: Callee<'_>) -> Result<usize> {
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
    name: Callee<'a>,
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

#[cfg(test)]
mod matcher_tests {
    //! The call matcher counts every valid call spelling, and only calls.
    //!
    //! The positive cases are the spellings that a first-argument or
    //! immediate-parenthesis matcher would miss; each one is a call that would
    //! otherwise hide a second packaging path from the shape assertions. The
    //! negative cases are the near-misses that must not count, so the matcher
    //! is not simply "contains the name".
    //!
    //! A default matcher may use a mod-level test block; see the harness use.

    use super::{Callee, count_calls};
    use rstest::rstest;

    /// The callee these cases count.
    ///
    /// Assembled rather than written as a literal so this module's own
    /// fixtures are not counted as calls by the tree scan: the scan reads this
    /// file too, and a literal here would be a false positive in the very
    /// assertion it describes.
    fn callee() -> String {
        ["package", "_sources"].concat()
    }

    /// Count that callee's calls in `source`.
    fn counted(source: &str) -> usize {
        count_calls(source, Callee::named(&callee()))
    }

    #[rstest]
    #[case::borrowed_first_argument(&format!("{}(&repository, target)", callee()), 1)]
    #[case::complete_borrowed_call(
        &format!("{}(&repository, target, invocation, budget)?;", callee()),
        1
    )]
    #[case::space_before_parenthesis(&format!("{} (&repository, target)", callee()), 1)]
    #[case::newline_before_argument(&format!("{}(\n        repository,", callee()), 1)]
    #[case::newline_before_parenthesis(&format!("{}\n(&repository, target)", callee()), 1)]
    #[case::turbofish(&format!("{}::<Budget>(repository,", callee()), 1)]
    #[case::in_a_match_arm(&format!("=> {}(repository, target)", callee()), 1)]
    fn a_call_is_counted_whatever_its_spelling(#[case] source: &str, #[case] expected: usize) {
        assert_eq!(
            counted(source),
            expected,
            "counting calls in {source:?} did not match the expected {expected}"
        );
    }

    #[rstest]
    #[case::declaration(&format!("    pub(super) fn {}(", callee()), 0)]
    #[case::import(&format!("use super::{{PACKAGING_INVOCATIONS, {}}};", callee()), 0)]
    #[case::prose_mention(&format!("// {} is pub(super) of package_set", callee()), 0)]
    #[case::other_callee("package_all(repository, target)", 0)]
    fn a_non_call_is_not_counted(#[case] source: &str, #[case] expected: usize) {
        assert_eq!(
            counted(source),
            expected,
            "counting calls in {source:?} did not match the expected {expected}"
        );
    }
}
