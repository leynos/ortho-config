//! Property coverage for `extends` chain ordering and repeatability.
//!
//! `selector_chain_proptest.rs` covers the explicit selector chain; this suite
//! covers the layered chain each selected file carries. The rule is one
//! sentence long — a file's `extends` chain contributes its ancestors first and
//! the file itself last, one layer per file, with the `extends` key stripped —
//! but `load_chain_for_file` builds it by recursion, and an accidental `rev()`,
//! a sort, or an off-by-one append is invisible to the example suite: that
//! suite pins chains of two and three fixed files, so a wrong order among three
//! can hide behind a two-file expectation.
//!
//! The reference model below is a direct transcription of the sentence. It
//! reads the generated names and nothing from `loader.rs`, so agreement is
//! evidence rather than restatement.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use ortho_config::load_config_file_as_chain;
use proptest::prelude::*;
use serde_json::Value;

#[path = "support/scoped_fixtures.rs"]
#[expect(dead_code, reason = "write_config serves sibling suites")]
mod scoped_fixtures;

use scoped_fixtures::write_body;

/// The shortest chain that is still a chain: a file and its parent.
const MIN_DEPTH: usize = 2;

/// The longest chain worth generating.
///
/// Depth is linear in file count, and each file costs two filesystem calls, so
/// a bound of six keeps a generated case cheap while still being deep enough
/// that a wrong order has several places to go wrong.
const MAX_DEPTH: usize = 6;

/// The per-file mark: `file-<index>.toml` writes `value = <index>`.
///
/// The value is the index rather than a constant so a chain layer can be
/// matched to the file it came from without reading paths, which is what makes
/// a reordering show up as a *value* mismatch rather than only a path mismatch.
fn body(index: usize) -> String {
    // `file-0` is the ancestor; each later file extends the one before it, so
    // loading the last file walks the whole chain back to `file-0`.
    if index == 0 {
        format!("value = {index}\n")
    } else {
        format!("value = {index}\nextends = \"file-{}.toml\"\n", index - 1)
    }
}

/// Write a `depth`-file chain, and return the path of its leaf.
///
/// The leaf is the file a caller would select; the chain it carries runs from
/// `file-0` to `file-{depth-1}`.
fn staged(root: &Path, depth: usize) -> Result<PathBuf> {
    for index in 0..depth {
        write_body(&root.join(format!("file-{index}.toml")), &body(index))
            .with_context(|| format!("write extends fixture {index}"))?;
    }
    Ok(root.join(format!("file-{}.toml", depth - 1)))
}

/// Assert `chain` is `depth` layers, ancestor-first, one file per layer.
fn assert_chain(chain: &ortho_config::FileLayerChain, depth: usize) -> Result<()> {
    let values = &chain.values;
    ensure!(
        values.len() == depth,
        "expected {depth} layers from a {depth}-file chain, got {}",
        values.len()
    );
    for (position, (value, path)) in values.iter().enumerate() {
        // Ancestor-first: the layer at `position` is `file-{position}`.
        let expected = format!("file-{position}.toml");
        ensure!(
            path.as_str().ends_with(expected.as_str()),
            "layer {position} should come from {expected}, got {path}"
        );
        ensure!(
            value.get("value").and_then(Value::as_u64) == u64::try_from(position).ok(),
            "layer {position} should carry value = {position}, got {value}"
        );
        ensure!(
            value.get("extends").is_none(),
            "layer {position} should not carry the extends key, got {value}"
        );
    }
    Ok(())
}

proptest! {
    /// A chain loads ancestor-first, one layer per file, in walk order.
    #[test]
    fn a_chain_loads_ancestor_first(depth in MIN_DEPTH..=MAX_DEPTH) {
        let temp = tempfile::tempdir().map_err(|err| TestCaseError::fail(
            format!("create temp dir: {err}"),
        ))?;
        let leaf = staged(temp.path(), depth).map_err(|err| TestCaseError::fail(
            format!("stage chain: {err:#}"),
        ))?;

        let chain = load_config_file_as_chain(&leaf)
            .map_err(|err| TestCaseError::fail(format!("load chain: {err}")))?
            .ok_or_else(|| TestCaseError::fail("leaf file must exist".to_owned()))?;
        assert_chain(&chain, depth).map_err(|err| TestCaseError::fail(format!("{err:#}")))?;
    }
}

/// A shared parent reached from two different leaves is not a false cycle.
///
/// `extends` names one parent, so a branch cannot occur inside a single chain.
/// It can occur across two: the scoped loader resolves one chain per candidate
/// file, so two candidates may each extend a common ancestor. The cycle guard
/// is a `HashSet` created per call, and if that ever became shared or permanent
/// the second leaf would be rejected as cyclic. Each leaf is therefore loaded
/// to completion here, after the other, in one process.
#[test]
fn a_shared_parent_is_not_a_cycle() -> Result<()> {
    let temp = tempfile::tempdir().context("create temp dir")?;
    let root = temp.path();
    write_body(&root.join("shared.toml"), "value = 0\n").context("write shared parent")?;
    for leaf in ["first.toml", "second.toml"] {
        write_body(
            &root.join(leaf),
            &format!("value = 1\nextends = \"shared.toml\"\n"),
        )
        .with_context(|| format!("write leaf {leaf}"))?;
    }

    for leaf in ["first.toml", "second.toml"] {
        let chain = load_config_file_as_chain(&root.join(leaf))
            .with_context(|| format!("load leaf {leaf}"))?
            .with_context(|| format!("leaf {leaf} must exist"))?;
        ensure!(
            chain.values.len() == 2,
            "leaf {leaf} should carry two layers (parent and itself), got {}",
            chain.values.len()
        );
        ensure!(
            chain.values[0].1.as_str().ends_with("shared.toml"),
            "leaf {leaf} should record the shared parent first, got {}",
            chain.values[0].1
        );
    }
    Ok(())
}
