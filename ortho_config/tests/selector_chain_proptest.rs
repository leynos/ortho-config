//! Property coverage for the ordered explicit selector chain.
//!
//! `scoped_stacking_proptest.rs` covers automatic composition; this suite
//! covers the rung above it. The rule under test is one sentence long — an
//! explicit chain is tried front to back, and the first rung that resolves a
//! value wins, suppressing every later rung and automatic discovery entirely —
//! but `ConfigPathSelector::resolve` decides it with a `find_map` over a
//! filtered iterator, and a reordering there is invisible to the example suite:
//! that suite pins single-rung cases, and its one three-rung chain only ever
//! populates the first two rungs.
//!
//! The reference model below is a direct transcription of the sentence. It
//! reads the generated marks and nothing from `policy.rs`, so agreement is
//! evidence rather than restatement.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ortho_config::{
    AutomaticMode, ConfigDiscovery, ConfigFilePolicy, ConfigPathSelector, MapEnv, MergeLayer,
};
use proptest::prelude::*;
use serde_json::Value;

#[path = "support/layer_assertions.rs"]
#[expect(dead_code, reason = "assert_layer_path serves sibling suites")]
mod layer_assertions;
#[path = "support/scoped_fixtures.rs"]
mod scoped_fixtures;

use layer_assertions::merge_layers;
use scoped_fixtures::write_config;

/// How many rungs a generated chain carries.
///
/// Three is the smallest length at which a wrong `rev()`, a wrong sort, or an
/// off-by-one bound changes the answer in more than one way, and it matches the
/// chain written by hand in `scoped_layers.rs`.
const RUNGS: usize = 3;

/// The value automatic discovery would contribute if it were not suppressed.
///
/// Distinct from every value a rung can write, so a selector that wrongly falls
/// through to automatic discovery is unmistakable rather than coincidentally
/// matching a selector file.
const AUTOMATIC_VALUE: u32 = 9001;

/// One rung's generated shape, as `(value, blank)`.
///
/// `value` is what the file writes, or `None` for a rung whose variable is left
/// unset. `blank` marks a rung whose variable *is* set but to the empty string,
/// which `ConfigPathSelector::resolve` treats as unset — the one mutation of
/// "unset" a naive implementation gets wrong, because the empty string resolves
/// to an empty `PathBuf` rather than to no path at all.
type Rung = (Option<u32>, bool);

/// A generated selector chain, and whether automatic discovery has anything.
#[derive(Clone, Debug)]
struct Plan {
    rungs: Vec<Rung>,
    automatic: bool,
}

fn any_plan() -> impl Strategy<Value = Plan> {
    let rung = (proptest::option::of(0u32..3u32), any::<bool>());
    (
        proptest::collection::vec(rung, RUNGS..=RUNGS),
        any::<bool>(),
    )
        .prop_map(|(rungs, automatic)| Plan { rungs, automatic })
}

/// Wrap a failed operation that renders through `Display`.
fn fail<E: std::fmt::Display>(what: &'static str) -> impl FnOnce(E) -> TestCaseError {
    move |err| TestCaseError::fail(format!("{what}: {err}"))
}

/// The variable name a rung's selector reads.
fn variable(index: usize) -> String {
    format!("CHAIN_RUNG_{index}")
}

/// The value `plan`'s first usable rung selects, or `None` if none does.
///
/// A blank rung is set-but-empty and therefore never wins, which is why it is
/// skipped here rather than treated as a resolving rung. A rung whose variable
/// is unset has no file to name and is skipped for the other reason.
fn predicted(plan: &Plan) -> Option<u32> {
    plan.rungs
        .iter()
        .find(|(value, blank)| value.is_some() && !*blank)
        .and_then(|(value, _)| *value)
        .or_else(|| plan.automatic.then_some(AUTOMATIC_VALUE))
}

/// Write `plan`'s selectable files into a fresh tree, and return both.
fn staged(plan: &Plan) -> Result<(tempfile::TempDir, PathBuf), TestCaseError> {
    let temp = tempfile::tempdir().map_err(fail("create temp dir"))?;
    let root = temp.path().to_path_buf();
    for (index, (value, blank)) in plan.rungs.iter().enumerate() {
        if *blank {
            continue;
        }
        if let Some(selected) = value {
            write_config(&root.join(format!("rung-{index}.toml")), *selected)
                .map_err(fail("write selector fixture"))?;
        }
    }
    if plan.automatic {
        write_config(&root.join("xdg/chain_app/config.toml"), AUTOMATIC_VALUE)
            .map_err(fail("write automatic fixture"))?;
    }
    Ok((temp, root))
}

/// The environment the policy consults, closed so the fixture decides it.
///
/// `XDG_CONFIG_HOME` and `HOME` both point into the tree, so the automatic leg
/// is a function of the plan rather than of the host's directories. `HOME` is
/// what the legacy `env_var` default would also read, which is why it is set
/// here even though the chain under test does not use it.
fn environment(root: &Path, plan: &Plan) -> MapEnv {
    let mut env = MapEnv::new()
        .with_var("XDG_CONFIG_HOME", root.join("xdg"))
        .with_var("HOME", root.join("home"));
    for (index, (value, blank)) in plan.rungs.iter().enumerate() {
        let contents = if *blank {
            // Set but empty: the case `resolve` has to filter.
            String::new()
        } else {
            match value {
                Some(_) => root
                    .join(format!("rung-{index}.toml"))
                    .display()
                    .to_string(),
                // An unset rung is simply absent from the map.
                None => continue,
            }
        };
        env = env.with_var(variable(index), contents);
    }
    env
}

/// Resolve the policy built from `plan` over an already-materialised tree.
fn resolve(root: &Path, plan: &Plan) -> ortho_config::FileLayerOutcome {
    let discovery = ConfigDiscovery::builder("chain_app")
        .config_file_name("config.toml")
        .clear_project_roots()
        .env_source(Arc::new(environment(root, plan)));
    let selectors = (0..plan.rungs.len())
        .map(|index| ConfigPathSelector::env(variable(index)))
        .collect::<Vec<_>>();
    ConfigFilePolicy::from_builder(discovery)
        .selectors(selectors)
        .automatic_mode(AutomaticMode::StackScopes)
        .resolve_layers()
}

/// The `value` a set of resolved layers folds to, if any layer carried one.
fn folded_value(layers: Vec<MergeLayer<'static>>) -> Option<u32> {
    merge_layers(layers)
        .get("value")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

proptest! {
    /// The chain is tried in declared order, and the first usable rung wins.
    #[test]
    fn the_first_usable_rung_wins(plan in any_plan()) {
        let (_temp, root) = staged(&plan)?;

        let outcome = resolve(&root, &plan);
        // Every rung either names an existing file or is blank or unset, so a
        // generated fixture never produces a failure on either mode.
        prop_assert!(
            outcome.selected_error().is_none(),
            "a well-formed fixture must not report a required failure: {:?}",
            outcome.selected_error()
        );
        prop_assert!(
            outcome.reportable_errors().is_empty(),
            "a well-formed fixture must not report optional failures: {:?}",
            outcome.reportable_errors()
        );

        let layers = outcome.into_result().map_err(fail("resolve layers"))?;
        prop_assert_eq!(
            folded_value(layers),
            predicted(&plan),
            "chain {:?} (automatic: {})",
            plan.rungs,
            plan.automatic
        );
    }

    /// A chain that resolves contributes exactly one layer.
    ///
    /// The winning rung names a single file, so a chain that resolves cannot
    /// stack: the count distinguishes "first rung wins" from "every rung with a
    /// readable file contributes", which a value-only assertion cannot, because
    /// the last writer of `value` would still be the first rung only by luck.
    #[test]
    fn a_resolving_chain_contributes_exactly_one_layer(plan in any_plan()) {
        let (_temp, root) = staged(&plan)?;

        let outcome = resolve(&root, &plan);
        let layers = outcome.into_result().map_err(fail("resolve layers"))?;
        let expected = usize::from(predicted(&plan).is_some());
        prop_assert_eq!(
            layers.len(),
            expected,
            "chain {:?} (automatic: {})",
            plan.rungs,
            plan.automatic
        );
    }
}
