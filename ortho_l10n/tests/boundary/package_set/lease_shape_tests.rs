//! Structural guarantees about how the packaging boundary is reached.
//!
//! These read the sources rather than run anything, because the property is
//! about the *shape* of the code: how many places can enter the packaging step,
//! and whether the set the lease protects is the set the contracts read. A
//! runtime observation of one contract cannot see a second call site in another
//! module, and the duplication this parameterises was exactly that - six
//! modules each calling `prepare_packaged_set`, invisible to each other, each
//! emptying a process-local cache in its own process.
//!
//! They are separated from [`super::lease_tests`] because that module's cases
//! drive the lease through real locks on a real packaging target. These read
//! source text, and the two that need somewhere to plant a file take the same
//! throwaway tree the lease cases use rather than a target with a lock on it.
//!
//! # Binding to calls, not substrings
//!
//! Each needle is spelled as a *call expression* - the callee followed by the
//! opening bracket and its first argument - so a doc comment that names the
//! function, a bare import, or the function's own definition do not count. A
//! bare name would, and a test that passes because a comment mentions the
//! function guards nothing. The needles are split with [`concat!`] so this
//! file's own source does not contain them and cannot count itself.
//!
//! These are shape checks, and they stay here because the alternative is
//! pretending a source-reading test can be behavioural. The behaviour those
//! call sites produce is pinned in [`super::lease_tests`] and
//! [`super::roots_tests`]; what these add is "and there is still exactly one of
//! them".

use crate::boundary::fixture_cargo::repository_root;
use crate::boundary::package_set::scratch::{Scratch, scratch_root};
use crate::boundary::package_set::{PACKAGED_CRATES, PACKAGING_INVOCATIONS};
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use rstest::{fixture, rstest};

/// The suite packages once per run, not once per contract.
///
/// Four separate shape claims, each about a different way the duplication
/// could come back or the recipe could stop producing what it promises:
///
/// - only `roots` may run a packaging invocation, so no contract can package
///   around the lease;
/// - that single call is `roots`' own, not one moved into a sibling;
/// - at least one contract actually calls `prepare_packaged_set`, so the socket
///   the boundary is reached through is not dead;
/// - the invocations between them package the whole packaged set, which is the
///   one relation between two constants that could drift silently.
///
/// A fifth call site to `prepare_packaged_set` is not a failure by itself -
/// the lease deduplicates it - but a call site that bypasses it and runs
/// packaging directly would restore the duplication. That is what this refuses.
#[rstest]
fn the_packaging_boundary_is_entered_once_per_run() -> Result<()> {
    let sources = repository_root()?.join("ortho_l10n/tests");
    let packaged = count_in_tree(&sources.join("boundary_tests"), "prepare_packaged_set")?;
    let invocations = PACKAGING_INVOCATIONS.len();

    // The packaging step runs from `roots` alone. Matched as a call with its
    // first argument, so the declaration, its import and any prose naming it
    // do not count as invocations.
    //
    // Scanned over the whole `boundary` tree rather than `package_set` alone,
    // because `package_sources` is `pub(super)` *of `package_set`*: the parent
    // scope is exactly what may call it, so a scan of `package_set` would miss
    // the very bypass it is meant to catch. `boundary_tests` cannot reach it,
    // and is not part of this tree.
    let packaging = count_in_tree(&sources.join("boundary"), "package_sources")?;
    ensure!(
        packaging == 1,
        "the packaging boundary is invoked from {packaging} places under \
         tests/boundary; it must be the single call in roots, or a contract can \
         package without the lease"
    );
    // The tree-wide count says "one call somewhere under boundary"; this says
    // "and that one is in roots, the module that holds the lease". Either alone
    // is weaker than it reads: the first admits a call moved out of `roots`
    // into a sibling, and the second would still pass with a second call
    // elsewhere in `package_set`. Together they leave no other place for it.
    let packaging_in_roots = count_in_roots(&sources)?;
    ensure!(
        packaging_in_roots == 1,
        "roots.rs makes {packaging_in_roots} packaging calls; the tree-wide scan \
         finds {packaging} under tests/boundary, so the single permitted call must \
         be roots' own, or the packaging step has moved out from under the lease"
    );
    ensure!(
        packaged > 0,
        "no contract calls prepare_packaged_set, so the packaging boundary is dead \
         and this contract proves nothing"
    );
    // The invocation count is a length in the type, so asserting a bound on it
    // would be a fact about the constant rather than about the suite. What can
    // drift is the relation *between* it and the packaged set: add a crate to
    // `PACKAGED_CRATES` without adding it to an invocation and the roots are
    // never produced, while every count-based assertion still passes.
    //
    // Sorted before comparing, because the two orders answer different
    // questions and are not meant to agree: `PACKAGED_CRATES` is in dependency
    // order for the reader, and the invocations are grouped by which edges each
    // has to patch. Comparing positionally would fail on a correct recipe.
    let mut invoked: Vec<&str> = PACKAGING_INVOCATIONS
        .iter()
        .flat_map(|invocation| invocation.crates.iter().copied())
        .collect();
    let mut declared: Vec<&str> = PACKAGED_CRATES
        .iter()
        .map(|(name, _version)| *name)
        .collect();
    invoked.sort_unstable();
    declared.sort_unstable();
    ensure!(
        invoked == declared,
        "the {invocations} packaging invocation(s) package {invoked:?} but the set \
         the contracts read is {declared:?}; a crate in one list and not the \
         other is either never packaged or never checked"
    );
    Ok(())
}

/// A call outside `package_set` is seen by the scan that guards the boundary.
///
/// The positive case above is only worth its line if the scan would *notice* a
/// second call site where the bypass would actually live: a sibling module
/// under `boundary`, which is the scope `package_sources`' own visibility
/// admits. Scanning `package_set` alone left that call invisible - the
/// guarantee was stated over a tree the bypass was not in - so this fixture
/// plants the call exactly there and requires the scan to find it.
///
/// The fixture writes into a throwaway tree under the boundary root, the same
/// place [`super::scratch`] puts its packaging targets, and not into any source
/// file: what is under test is the scan's reach, not any file the checkout
/// already holds. The tree is a `TempDir`, so a case leaves nothing behind.
#[rstest]
fn a_call_outside_package_set_is_counted(scratch_sources: Result<ScratchSources>) -> Result<()> {
    let scratch = scratch_sources?;
    let sources = scratch.path();
    let bypass = sources.join("boundary/consumer/mod.rs");
    let parent = bypass
        .parent()
        .context("the planted bypass has no parent")?;
    let directory = Dir::open_ambient_dir(parent, cap_std::ambient_authority())
        .with_context(|| format!("open {parent}"))?;
    directory
        .write(
            "mod.rs",
            concat!(
                "fn bypass(repository: &Utf8Path, target: &Utf8Path) {\n",
                "    let _ = package_sources",
                "(repository, target, &INVOCATION, budget);\n",
                "}\n",
            ),
        )
        .with_context(|| format!("plant a bypass call at {bypass}"))?;

    let counted = count_in_tree(&sources.join("boundary"), "package_sources")?;
    ensure!(
        counted == 1,
        "a packaging call planted at boundary/consumer/mod.rs was counted {counted} \
         times by the scan; a bypass outside package_set is invisible to it, which \
         is the whole guarantee the positive case claims"
    );
    Ok(())
}

/// A scratch sources tree with the directories the shape scans walk.
///
/// Owns the scratch root as well as the tree inside it: the scan needs the
/// `.rs` file the case plants to still be there, and a fixture that returned a
/// path while dropping the `TempDir` that anchors it would hand the case a
/// directory that has already been removed.
struct ScratchSources {
    /// Kept for its destructor, which removes the tree below it. Never read,
    /// hence the underscore: the point of the field is its lifetime, not its
    /// value.
    _scratch: Scratch,
    /// The sources tree the scans are pointed at.
    sources: Utf8PathBuf,
}

impl ScratchSources {
    /// The sources tree the shape scans walk.
    fn path(&self) -> &Utf8Path {
        &self.sources
    }
}

/// Build a [`ScratchSources`] tree for a case that plants its own sources.
///
/// Real repository sources are deliberately not used, so a case can plant a
/// bypass without writing to any tracked file. The tree itself lives under the
/// boundary root, beside the packaging targets, and is removed on drop.
#[fixture]
fn scratch_sources(scratch_root: Result<Scratch>) -> Result<ScratchSources> {
    let scratch = scratch_root?;
    let sources = scratch.path().join("sources");
    Dir::create_ambient_dir_all(&sources, cap_std::ambient_authority())
        .with_context(|| format!("create the scratch sources tree {sources}"))?;
    let directory = Dir::open_ambient_dir(&sources, cap_std::ambient_authority())
        .with_context(|| format!("open the scratch sources tree {sources}"))?;
    for relative in ["boundary/consumer", "boundary/package_set"] {
        directory
            .create_dir_all(relative)
            .with_context(|| format!("create {relative} in the scratch sources tree"))?;
    }
    Ok(ScratchSources {
        _scratch: scratch,
        sources,
    })
}

/// How many packaging calls `roots.rs` itself makes.
///
/// A single-file read rather than a tree walk, because the question here is
/// about that one module. [`count_in_tree`] opens its root as a directory, so
/// it cannot answer it; the two together are what pin the call to `roots`.
fn count_in_roots(sources: &Utf8Path) -> Result<usize> {
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
fn count_calls(source: &str, name: &str) -> usize {
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
fn count_in_tree(root: &Utf8Path, name: &str) -> Result<usize> {
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

/// The packaged set the lease protects is the one the contracts read.
///
/// A lease over a directory nothing reads would be a lock around nothing. This
/// ties the two together through the *call* that builds the roots: `roots.rs`
/// must reach the leased step through `prepare(`, and it must pass the same
/// target the packaged roots are derived from. A rename on either side fails
/// here rather than leaving a lock that no reader ever contends for.
///
/// The set itself is read from source rather than asserted non-empty:
/// [`PACKAGED_CRATES`] is a fixed-length array, so "not empty" is a fact rustc
/// checks, not one this case could ever report.
#[rstest]
fn the_lease_protects_the_set_the_contracts_read() -> Result<()> {
    let roots = repository_root()?.join("ortho_l10n/tests/boundary/package_set/roots.rs");
    let directory = Dir::open_ambient_dir(
        roots
            .parent()
            .context("the packaged roots module has no parent")?,
        cap_std::ambient_authority(),
    )
    .with_context(|| format!("open the directory containing {roots}"))?;
    let source = directory
        .read_to_string("roots.rs")
        .with_context(|| format!("read {roots}"))?;

    // The leased step, as a call expression rather than a mention. The guard is
    // the first argument because `prepare` requires the proof that it is held,
    // so the shape pinned here is `prepare(guard, &target, ..)`.
    let prepares = source
        .matches(concat!("prepare", "(guard, &target"))
        .count();
    ensure!(
        prepares == 1,
        "roots.rs reaches the lease through {prepares} `prepare(guard, &target, ..)` \
         calls; it must be exactly one, and the packaged roots must be built from \
         the same target the lease protects"
    );

    // The roots the contracts read are derived from that leased target, so a
    // rename of one without the other cannot leave the lock pointed elsewhere.
    ensure!(
        source.contains(concat!("target", ".join(PACKAGE_DIRECTORY)")),
        "roots.rs no longer derives the unpacked roots from the leased packaging \
         target; the lease would be protecting a directory nothing reads"
    );
    Ok(())
}
