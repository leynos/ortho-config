//! Opt-in scope stacking for automatic discovery candidates.
//!
//! [`AutomaticMode::StackScopes`] resolves every requested scope and
//! concatenates the results, so a project file can build on a user file rather
//! than replacing it. Two orderings meet in this module and they run in
//! opposite directions, so each is named here rather than left implicit:
//!
//! * The candidate list is a **preference order**. It is returned
//!   most-preferred first, because index 0 is the location the historic
//!   first-wins scan selects.
//! * A composed layer list is a **precedence order**. A `MergeComposer` applies
//!   layers in the order given and the last one wins, so a later position means
//!   a higher-precedence value.
//!
//! A scope therefore walks its candidates in *reverse* preference order, which
//! is what maps the two onto each other: the least-preferred location is
//! applied first and the most-preferred is applied last, so the latter still
//! wins. That keeps "later applied wins" as the single rule for the whole
//! system while preserving the historic choice *within* a scope. Emitting
//! candidates in preference order instead would let a fallback such as
//! `~/.demo.toml` silently override `$XDG_CONFIG_HOME/demo/config.toml`,
//! inverting established behaviour the moment a second location starts
//! loading.
//!
//! [`AutomaticMode::FirstWins`]: crate::AutomaticMode::FirstWins
//! [`AutomaticMode::StackScopes`]: crate::AutomaticMode::StackScopes

use std::collections::HashSet;
use std::path::PathBuf;

use crate::MergeLayer;
use crate::file::canonicalise;

use super::candidate_set::{Candidate, CandidateSet};
use super::load::{CandidateFailure, PartitionedErrors};
use super::telemetry;
use super::{AutomaticMode, ConfigDiscovery, DiscoveryLayersOutcome, DiscoveryScope};

/// Every layer one scope contributes, with that scope's diagnostics.
struct ScopeLayers {
    layers: Vec<MergeLayer<'static>>,
    errors: PartitionedErrors,
}

impl ConfigDiscovery {
    /// Compose automatic file layers according to an explicit scope policy.
    ///
    /// [`AutomaticMode::FirstWins`] preserves [`Self::compose_layers`] exactly:
    /// the historic scan takes the first candidate that loads and stops.
    ///
    /// [`AutomaticMode::StackScopes`] loads every requested scope and appends
    /// their layers in scope order, so later scopes override earlier ones
    /// through `MergeComposer`'s existing last-pushed-wins semantics. Each
    /// scope contributes every applicable candidate that loads, not only its
    /// first success, and a scope's layers are emitted least-preferred first —
    /// see the module documentation for why that is the same rule as "later
    /// scopes win" and not its opposite.
    ///
    /// [`AutomaticMode::FirstWins`]: crate::AutomaticMode::FirstWins
    /// [`AutomaticMode::StackScopes`]: crate::AutomaticMode::StackScopes
    pub fn compose_scoped_layers(
        &self,
        mode: AutomaticMode,
        scopes: &[DiscoveryScope],
    ) -> DiscoveryLayersOutcome {
        if matches!(mode, AutomaticMode::FirstWins) {
            return self.compose_layers();
        }

        telemetry::attempt(telemetry::OPERATION_COMPOSE_LAYERS);
        let set = self.candidate_set();
        set.decisions.emit();
        let mut errors = PartitionedErrors::default();
        let mut layers = Vec::new();
        let mut loaded_paths = HashSet::new();

        for scope in scopes {
            let scope_layers = Self::compose_scope(*scope, &set, &mut loaded_paths);
            errors.append(scope_layers.errors);
            layers.extend(scope_layers.layers);
        }

        telemetry::load_outcome(
            telemetry::OPERATION_COMPOSE_LAYERS,
            if layers.is_empty() {
                telemetry::OUTCOME_NOT_FOUND
            } else {
                telemetry::OUTCOME_SUCCESS
            },
            None,
        );
        errors.into_layers_outcome(layers)
    }

    /// Resolve one scope, least-preferred candidate first.
    ///
    /// Every applicable candidate is attempted rather than only the first that
    /// loads: that *is* the mode, so a project file can layer over a user file
    /// instead of replacing it. Attempting all of them is also what makes the
    /// diagnostics complete, since a candidate's defect is only discoverable by
    /// opening it.
    ///
    /// The scope's terminal event is emitted once, after the walk, and names the
    /// effective winner: the most-preferred candidate that loaded, which is the
    /// last one applied and so the one whose values survive.
    fn compose_scope(
        scope: DiscoveryScope,
        set: &CandidateSet,
        loaded_paths: &mut HashSet<PathBuf>,
    ) -> ScopeLayers {
        let mut errors = PartitionedErrors::default();
        let mut winner = None;
        let mut layers = Vec::new();

        for (index, candidate) in Self::scope_candidates(scope, set) {
            let required = Self::is_required_candidate(index, set.required_bound);
            match Self::chain_layers(&candidate.path, required) {
                Ok(Some(chain)) => {
                    layers.extend(Self::unique_layers(chain, loaded_paths));
                    winner = Some(candidate.source);
                }
                Ok(None) => {}
                Err(error) => errors.record(
                    &CandidateFailure {
                        operation: telemetry::OPERATION_COMPOSE_LAYERS,
                        required,
                        source: candidate.source,
                    },
                    error,
                ),
            }
        }

        if let Some(source) = winner {
            telemetry::load_outcome(
                telemetry::OPERATION_COMPOSE_LAYERS,
                telemetry::OUTCOME_SUCCESS,
                Some(source),
            );
        }
        ScopeLayers { layers, errors }
    }

    /// The scope's candidates, least-preferred first.
    ///
    /// The reversal happens here, in one named place, so that no call site has
    /// to hold two opposite orderings in mind: everything downstream of this
    /// function merely appends in application order.
    fn scope_candidates<'set>(
        scope: DiscoveryScope,
        set: &'set CandidateSet,
    ) -> impl Iterator<Item = (usize, &'set Candidate)> {
        set.candidates
            .iter()
            .enumerate()
            .rev()
            .filter(move |(_, candidate)| candidate.scope == Some(scope))
    }

    fn unique_layers(
        chain: Vec<MergeLayer<'static>>,
        loaded_paths: &mut HashSet<PathBuf>,
    ) -> Vec<MergeLayer<'static>> {
        chain
            .into_iter()
            .filter(|layer| Self::record_first_canonical_path(loaded_paths, layer))
            .collect()
    }

    /// Keep the first-loaded copy of a successfully loaded file.
    ///
    /// The loader canonicalises every chain path, so canonical identity also
    /// collapses aliases and symlinks without changing public layer metadata.
    /// "First" is application order — scopes in `scope_order`, and within a
    /// scope the least-preferred candidate — which is what stops a file
    /// reachable from two places contributing two layers and silently doubling
    /// append-strategy vectors.
    fn record_first_canonical_path(
        loaded_paths: &mut HashSet<PathBuf>,
        layer: &MergeLayer<'static>,
    ) -> bool {
        layer.path().is_none_or(|path| {
            canonicalise(path.as_std_path())
                .map_or(true, |canonical| loaded_paths.insert(canonical))
        })
    }
}
