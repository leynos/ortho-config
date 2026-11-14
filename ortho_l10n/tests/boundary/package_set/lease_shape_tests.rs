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
//! drive the lease through real locks and temporary directories; these never
//! touch either.
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
use crate::boundary::package_set::{PACKAGED_CRATES, PACKAGING_INVOCATIONS};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::fs_utf8::Dir;
use rstest::rstest;

/// The suite packages once per run, not once per contract.
///
/// Three separate shape claims, each about a different way the duplication
/// could come back:
///
/// - only `roots` may run a packaging invocation, so no contract can package
///   around the lease;
/// - the invocation list stays a small, fixed pair, because the pair is the
///   whole run's packaging cost and a third entry would not fit the per-test
///   allowance;
/// - at least one contract actually calls `prepare_packaged_set`, so the socket
///   the boundary is reached through is not dead.
///
/// A fifth call site to `prepare_packaged_set` is not a failure by itself -
/// the lease deduplicates it - but a call site that bypasses it and runs
/// packaging directly would restore the duplication. That is what this refuses.
#[rstest]
fn the_packaging_boundary_is_entered_once_per_run() -> Result<()> {
    let sources = repository_root()?.join("ortho_l10n/tests");
    let packaged = count_in_tree(
        &sources.join("boundary_tests"),
        concat!("prepare_packaged_set", "("),
    )?;
    let invocations = PACKAGING_INVOCATIONS.len();

    // The packaging step runs from `roots` alone. Matched as a call with its
    // first argument, so the declaration, its import and any prose naming it
    // do not count as invocations.
    let packaging = count_in_tree(
        &sources.join("boundary/package_set"),
        concat!("package_sources", "(repository"),
    )?;
    ensure!(
        packaging == 1,
        "the packaging boundary is invoked from {packaging} places under \
         tests/boundary/package_set; it must be the single call in roots, or a \
         contract can package without the lease"
    );
    ensure!(
        invocations > 0 && invocations <= 2,
        "the suite performs {invocations} packaging invocations per run; the lease \
         makes that pair the whole run's packaging cost, and a third invocation \
         would not fit the per-test allowance"
    );
    ensure!(
        packaged > 0,
        "no contract calls prepare_packaged_set, so the packaging boundary is dead \
         and this contract proves nothing"
    );
    Ok(())
}

/// How many times `needle` appears in the `.rs` files under `root`.
///
/// Counts occurrences rather than files, because two call sites in one module
/// are exactly the duplication this looks for.
fn count_in_tree(root: &Utf8Path, needle: &str) -> Result<usize> {
    let root_directory = Dir::open_ambient_dir(root, cap_std::ambient_authority())
        .with_context(|| format!("open {root}"))?;
    let mut total = 0;
    let mut pending = vec![(root_directory, String::new())];
    while let Some((directory, prefix)) = pending.pop() {
        for listed in directory
            .entries()
            .with_context(|| format!("read {root}/{prefix}"))?
        {
            let entry = listed.with_context(|| format!("read an entry under {root}/{prefix}"))?;
            let name = entry
                .file_name()
                .with_context(|| format!("read a file name under {root}/{prefix}"))?;
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let file_type = entry
                .file_type()
                .with_context(|| format!("read the type of {root}/{relative}"))?;
            if file_type.is_dir() {
                pending.push((entry.open_dir().context("open a subdirectory")?, relative));
                continue;
            }
            let is_rust = Utf8Path::new(&name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"));
            if !file_type.is_file() || !is_rust {
                continue;
            }
            // Relative to the directory the entry came from, not to the root:
            // `directory` is the parent, so only the final name is a path in it.
            let text = directory
                .read_to_string(&name)
                .with_context(|| format!("read {root}/{relative}"))?;
            total += text.matches(needle).count();
        }
    }
    Ok(total)
}

/// The packaged set the lease protects is the one the contracts read.
///
/// A lease over a directory nothing reads would be a lock around nothing. This
/// ties the two together through the *call* that builds the roots: `roots.rs`
/// must reach the leased step through `prepare(`, and it must pass the same
/// target the packaged roots are derived from. A rename on either side fails
/// here rather than leaving a lock that no reader ever contends for.
#[rstest]
fn the_lease_protects_the_set_the_contracts_read() -> Result<()> {
    ensure!(
        !PACKAGED_CRATES.is_empty(),
        "the packaged set is empty, so the lease protects nothing"
    );
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

    // The leased step, as a call expression rather than a mention.
    let prepares = source.matches(concat!("prepare", "(&target")).count();
    ensure!(
        prepares == 1,
        "roots.rs reaches the lease through {prepares} `prepare(&target, ..)` calls; \
         it must be exactly one, and the packaged roots must be built from the same \
         target the lease protects"
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
