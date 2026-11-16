//! ID-keyed lookup of the supplied manifests.
//!
//! Both duplicate detection and example ownership need to answer "how many
//! manifests declare this ID, and where is the first one?" without rescanning
//! the list for each claim, so the map is built once per validation call.

use std::collections::BTreeMap;

use super::super::input::ManifestClaims;

/// How an example's owning manifest ID resolved.
pub(super) enum OwnerKey {
    /// Exactly one manifest declares the ID, at this input position.
    Unique(usize),
    /// More than one manifest declares the ID; the first position is kept.
    Ambiguous(usize),
    /// No manifest declares the ID.
    Missing,
}

/// An ID-keyed view of the supplied manifests.
pub(super) struct ManifestIndex<'a> {
    /// Manifest positions per ID, in input order.
    positions: BTreeMap<&'a str, Vec<usize>>,
    /// Supplied manifests.
    pub(super) manifests: &'a [ManifestClaims],
}

impl<'a> ManifestIndex<'a> {
    /// Builds the index from the supplied manifests.
    pub(super) fn new(manifests: &'a [ManifestClaims]) -> Self {
        let mut positions: BTreeMap<&'a str, Vec<usize>> = BTreeMap::new();
        for (position, manifest) in manifests.iter().enumerate() {
            positions
                .entry(manifest.id.as_str())
                .or_default()
                .push(position);
        }
        Self {
            positions,
            manifests,
        }
    }

    /// Resolves an example's owner ID to an input position.
    pub(super) fn owner(&self, id: &str) -> OwnerKey {
        match self.positions.get(id).map(Vec::as_slice) {
            None => OwnerKey::Missing,
            Some([position]) => OwnerKey::Unique(*position),
            Some(positions) => positions
                .first()
                .map_or(OwnerKey::Missing, |position| OwnerKey::Ambiguous(*position)),
        }
    }

    /// Returns whether the manifest at `position` repeats an earlier ID.
    pub(super) fn is_repeat(&self, id: &str, position: usize) -> bool {
        self.positions
            .get(id)
            .and_then(|positions| positions.first())
            .is_some_and(|first| *first != position)
    }
}
