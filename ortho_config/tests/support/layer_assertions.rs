//! Layer assertions shared by the scoped-discovery test suites.
//!
//! Split from `scoped_layers.rs` so each suite stays within the repository's
//! 400-line code file ceiling, and so the canonicalising path assertion has one
//! definition rather than one per suite.

use std::path::Path;

use anyhow::{Result, anyhow, ensure};
use ortho_config::declarative::merge_value;

/// Assert that a layer came from `expected`, comparing canonical forms.
///
/// The loader stores the canonicalised path — on Windows `dunce::canonicalize`
/// rewrites the representation of an otherwise identical path, so comparing the
/// raw fixture path fails there for a reason that has nothing to do with layer
/// selection. Both paths are printed on failure, and the expectation itself is
/// canonicalised, so the assertion stays about *which file won*.
///
/// # Errors
///
/// Returns an error when the layer names a different file, or when either path
/// cannot be canonicalised.
pub fn assert_layer_path(layer: &ortho_config::MergeLayer<'_>, expected: &Path) -> Result<()> {
    let actual = layer
        .path()
        .ok_or_else(|| anyhow!("layer has no path, expected {}", expected.display()))?
        .as_std_path()
        .to_path_buf();
    let shown = actual.display().to_string();
    let actual = ortho_config::file::canonicalise(&actual)
        .map_err(|err| anyhow!("canonicalise layer path {shown}: {err}"))?;
    let expected_canonical = ortho_config::file::canonicalise(expected)
        .map_err(|err| anyhow!("canonicalise expected path {}: {err}", expected.display()))?;
    ensure!(
        actual == expected_canonical,
        "expected layer from {expected}, got {shown} (canonicalised: {actual} vs {expected_canonical})",
        expected = expected.display(),
        actual = actual.display(),
        expected_canonical = expected_canonical.display(),
    );
    Ok(())
}

/// Merge layers into one value, in application order.
///
/// Later layers override earlier ones, matching how a caller folds the result,
/// so the surviving keys are the effective winner's.
pub fn merge_layers(layers: Vec<ortho_config::MergeLayer<'static>>) -> serde_json::Value {
    let mut merged = serde_json::Value::Null;
    for layer in layers {
        merge_value(&mut merged, layer.into_value());
    }
    merged
}
