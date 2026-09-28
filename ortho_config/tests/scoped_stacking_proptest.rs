//! Property coverage for scope-stacked automatic file composition.
//!
//! The example-based suite in `scoped_stacking.rs` pins one scenario at a time;
//! each property below states the invariant those examples are instances of.
//! Files are written with `support/scoped_fixtures.rs` and checked with
//! `support/layer_assertions.rs`.
//!
//! The tree is a fixed vocabulary of locations plus one alias rung: a plan
//! chooses which exist, and in which scope order, because the shape of an
//! automatic candidate list comes from the discovery configuration rather than
//! the generator. Failures propagate with `?` from helpers returning
//! `Result<_, TestCaseError>`: `prop_assert!` returns an error rather than
//! unwinding, so the denied panicking and unwrapping lints are unreachable.
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ortho_config::{
    AutomaticMode, ConfigDiscovery, ConfigDiscoveryBuilder, ConfigFilePolicy, DiscoveryScope,
    MapEnv, MergeLayer,
};
use proptest::prelude::*;
use serde_json::{Value, json};

#[path = "support/layer_assertions.rs"]
mod layer_assertions;
#[path = "support/scoped_fixtures.rs"]
#[expect(dead_code, reason = "write_config serves sibling suites")]
mod scoped_fixtures;

use layer_assertions::{assert_layer_path, merge_layers};
use scoped_fixtures::write_body;

/// The scopes generated over; each is requested at most once per plan.
const SCOPES: [DiscoveryScope; 3] = [
    DiscoveryScope::System,
    DiscoveryScope::User,
    DiscoveryScope::Project,
];

/// One location a generated plan can offer the assembler, as
/// `(scope, rank, value, unique key, alias)`.
///
/// `scope` and `rank` describe the discovery configuration, not the generated
/// data; writing them down here is what lets the reference model be read against
/// `docs/design.md` rather than `discovery::scoped`. `alias` marks the project
/// root spelled `nested/..`: candidate *assembly* de-duplicates on the literal
/// `OsString` (`ConfigDiscovery::dedup_key`), so two spellings of one directory
/// survive as two candidates and only the canonical-path filter collapses them.
///
/// `scope` indexes [`SCOPES`] and `rank` is the preference within it, 0 being
/// the most preferred. `value` is what the file writes for `value`, and the
/// unique key is set by that file alone, so a merge shows if it contributed.
type Rung = (usize, usize, i32, &'static str, bool);

/// Every rung a plan can offer, in the order its marks are given.
///
/// The five rows are, in turn: the only system location; the most- and
/// least-preferred user locations; and one project file reached under two
/// spellings, the second being the project root written as `nested/..`.
const VOCABULARY: [Rung; 5] = [
    (0, 0, 0, "ks", false),
    (1, 0, 1, "k0", false),
    (1, 1, 2, "k1", false),
    (2, 0, 3, "kp", false),
    (2, 1, 3, "kp", true),
];

/// Whether a rung is the project root spelled with `nested/..`.
///
/// The last field of [`Rung`], named here so the filter below reads as prose.
const fn is_alias(rung: Rung) -> bool {
    let (_, _, _, _, alias) = rung;
    alias
}

/// The path one location names, under `root`.
///
/// `$HOME/.config/demo/config.toml` and `$XDG_CONFIG_HOME/.demo.toml` are more
/// preferred than the dotfile rung, but no plan writes either, and an absent
/// candidate cannot change the winner.
fn candidate_path(root: &Path, rung: Rung) -> PathBuf {
    let (scope, rank, _, _, alias) = rung;
    if alias {
        return root.join("project/nested/../project.toml");
    }
    match (scope, rank) {
        (0, _) => root.join("system/demo/config.toml"),
        (1, 0) => root.join("user/xdg/demo/config.toml"),
        (1, _) => root.join("user/home/.demo.toml"),
        _ => root.join("project/project.toml"),
    }
}

/// One generated composition: which rungs exist, in which scope order.
///
/// The rungs are a vector of pairs rather than five `bool` fields, which would
/// be a state machine in disguise (`clippy::struct_excessive_bools`).
#[derive(Clone, Debug)]
struct Plan {
    /// Every offered rung, in [`VOCABULARY`] order, with its existence.
    located: Vec<(Rung, bool)>,
    /// Whether the project root's `nested/..` spelling is offered.
    alias: bool,
    /// A permutation of `0..SCOPES.len()`.
    order: Vec<usize>,
}

/// Build a plan from one mark per [`VOCABULARY`] rung, plus its scope order.
///
/// Both project rungs name one file, so the project mark is their shared
/// existence: the alias rung marks a second *spelling* of that root, and is
/// offered as a candidate only when that spelling is marked.
fn plan_from(marks: [bool; 5], order: Vec<usize>) -> Plan {
    let [system, xdg, dotfile, project, alias] = marks;
    let located = VOCABULARY
        .into_iter()
        .zip([system, xdg, dotfile, project, project])
        .filter(|(rung, _)| alias || !is_alias(*rung))
        .collect();
    Plan {
        located,
        alias,
        order,
    }
}

/// Any composition: which rungs exist, plus a shuffled scope order.
///
/// The marks are generated as an array and destructured rather than indexed:
/// `clippy::indexing_slicing` is denied workspace-wide.
fn any_plan() -> impl Strategy<Value = Plan> {
    (any::<[bool; 5]>(), Just(vec![0usize, 1, 2]).prop_shuffle())
        .prop_map(|(marks, order)| plan_from(marks, order))
}

/// A plan offering the alias rung, with the file it names actually written.
///
/// Forced rather than filtered: a `prop_filter` here would reject three cases in
/// four, spending the run's rejection budget for no extra coverage. The project
/// mark is forced with the alias mark, because the alias rung is a spelling of
/// the project file and offering it alone would name a file no plan writes.
fn any_aliased_plan() -> impl Strategy<Value = Plan> {
    (any::<[bool; 3]>(), Just(vec![0usize, 1, 2]).prop_shuffle()).prop_map(|(marks, order)| {
        let [system, xdg, dotfile] = marks;
        plan_from([system, xdg, dotfile, true, true], order)
    })
}

/// A discovery over `root`, with every consulted variable injected.
///
/// Closing the environment is what makes the candidate list a function of the
/// plan alone; nothing here reads the host's own directories.
fn builder(root: &Path, plan: &Plan) -> ConfigDiscoveryBuilder {
    let env = MapEnv::new()
        .with_var("XDG_CONFIG_DIRS", root.join("system"))
        .with_var("XDG_CONFIG_HOME", root.join("user/xdg"))
        .with_var("HOME", root.join("user/home"));
    let builder = ConfigDiscovery::builder("demo")
        .config_file_name("config.toml")
        .dotfile_name(".demo.toml")
        .project_file_name("project.toml")
        .clear_project_roots()
        .add_project_root(root.join("project"))
        .env_source(Arc::new(env));
    if plan.alias {
        // The second rung names the same directory as `nested/..`; assembly keys
        // on the literal spelling, so both rungs survive to the loader.
        builder.add_project_root(root.join("project/nested/.."))
    } else {
        builder
    }
}

/// Wrap a failed operation that renders through `Display`.
fn fail<E: std::fmt::Display>(what: &'static str) -> impl FnOnce(E) -> TestCaseError {
    move |err| TestCaseError::fail(format!("{what}: {err}"))
}

/// Write `plan`'s existing locations into a fresh tree, and return both.
///
/// `nested` is created even when no alias rung is offered, because the aliased
/// root spells the project directory `project/nested/..`; without it that root
/// could not be opened, and the aliased candidate would be reported absent for a
/// reason unrelated to the generated marks.
fn staged(plan: &Plan) -> Result<(tempfile::TempDir, PathBuf), TestCaseError> {
    let temp = tempfile::tempdir().map_err(fail("create temp dir"))?;
    let root = temp.path().to_path_buf();
    write_body(&root.join("project/nested/.keep"), "").map_err(fail("create alias directory"))?;
    for (rung, exists) in &plan.located {
        if !exists {
            continue;
        }
        let (_, _, value, unique, _) = rung;
        let body = format!("value = {value}\n{unique} = {value}\n");
        write_body(&candidate_path(&root, *rung), &body).map_err(fail("write fixture"))?;
    }
    Ok((temp, root))
}

/// Compose the stacked layers for `plan` over an already-materialised tree.
fn compose(root: &Path, plan: &Plan) -> Result<Vec<MergeLayer<'static>>, TestCaseError> {
    let outcome = builder(root, plan)
        .build()
        .compose_scoped_layers(AutomaticMode::StackScopes, &requested(plan));
    if !outcome.required_errors.is_empty() || !outcome.optional_errors.is_empty() {
        return Err(TestCaseError::fail(format!(
            "generated files are all optional and well formed, got {:?} {:?}",
            outcome.required_errors, outcome.optional_errors
        )));
    }
    Ok(outcome.value)
}

/// The requested scope order, as the public scope values.
fn requested(plan: &Plan) -> Vec<DiscoveryScope> {
    plan.order
        .iter()
        .filter_map(|index| SCOPES.get(*index).copied())
        .collect()
}

/// The scopes holding at least one offered location that exists.
///
/// The scopes occupy disjoint directories, so an existing location always
/// survives canonical de-duplication, and a scope is an origin exactly when it
/// appears here.
fn contributing(plan: &Plan) -> HashSet<usize> {
    plan.located
        .iter()
        .filter(|(_, exists)| *exists)
        .map(|((scope, ..), _)| *scope)
        .collect()
}

/// The locations that exist, in the order a `MergeComposer` applies them.
///
/// A second implementation of the precedence rule, transcribed from
/// `docs/design.md`: scopes are walked in the requested order, because "scope
/// order is precedence order", and within a scope the least-preferred location
/// is applied first. It reads only the generated marks and the table above,
/// never `scope_candidates` or `chain_layers`, so agreement is evidence rather
/// than restatement.
fn application_order(plan: &Plan) -> Vec<Rung> {
    let mut ordered = Vec::new();
    for scope in &plan.order {
        let mut in_scope = plan
            .located
            .iter()
            .filter(|(rung, exists)| rung.0 == *scope && *exists)
            .map(|(rung, _)| *rung)
            .collect::<Vec<_>>();
        in_scope.sort_by_key(|(_, rank, ..)| std::cmp::Reverse(*rank));
        ordered.extend(in_scope);
    }
    ordered
}

/// The reference model's prediction: canonical layer paths in application
/// order, and the value those layers fold to.
///
/// Pins the rule in `ConfigDiscovery::record_first_canonical_path`: a canonical
/// path already recorded is dropped, so the *first* occurrence in application
/// order survives, and later application order wins the merge. Fails if a file
/// cannot be canonicalised, meaning fixture and plan disagree.
fn expected(root: &Path, plan: &Plan) -> Result<(Vec<PathBuf>, Value), TestCaseError> {
    let mut seen = HashSet::new();
    let mut paths = Vec::new();
    let mut merged: BTreeMap<&str, i32> = BTreeMap::new();
    for rung in application_order(plan) {
        let (_, _, value, unique, _) = rung;
        let path = candidate_path(root, rung);
        let canonical = ortho_config::file::canonicalise(&path).map_err(|err| {
            TestCaseError::fail(format!("canonicalise {}: {err}", path.display()))
        })?;
        if seen.insert(canonical.clone()) {
            paths.push(canonical);
        }
        merged.insert(unique, value);
        merged.insert("value", value);
    }
    // No layers at all means no merged value, matching `merge_layers`.
    let value = if merged.is_empty() {
        Value::Null
    } else {
        Value::Object(
            merged
                .into_iter()
                .map(|(key, val)| (key.to_owned(), json!(val)))
                .collect(),
        )
    };
    Ok((paths, value))
}

/// The scope whose file supplied the surviving `value`, if any did.
///
/// Each file tags `value` with its own field, and no two scopes share a tag, so
/// the merged scalar names the winning scope.
fn winning_scope(layers: Vec<MergeLayer<'static>>) -> Option<usize> {
    let tag = merge_layers(layers).get("value")?.as_i64()?;
    Some(match tag {
        0 => 0,
        1 | 2 => 1,
        _ => 2,
    })
}

proptest! {
    /// Layer contents and order are the reference model's prediction.
    #[test]
    fn composed_layers_match_the_reference_model(plan in any_plan()) {
        let (_temp, root) = staged(&plan)?;

        let (paths, value) = expected(&root, &plan)?;
        let layers = compose(&root, &plan)?;
        prop_assert_eq!(layers.len(), paths.len(), "layer count");
        for (layer, path) in layers.iter().zip(paths.iter()) {
            assert_layer_path(layer, path).map_err(fail("layer path"))?;
        }
        prop_assert_eq!(merge_layers(layers), value);
    }

    /// Every scope with an existing location is an origin, and no other is.
    #[test]
    fn origins_report_exactly_the_contributing_scopes(plan in any_plan()) {
        let (_temp, root) = staged(&plan)?;

        let outcome = ConfigFilePolicy::from_builder(builder(&root, &plan))
            .automatic_mode(AutomaticMode::StackScopes)
            .scope_order(requested(&plan))
            .resolve_layers();

        let reported = outcome
            .origins()
            .iter()
            .filter_map(|scope| SCOPES.iter().position(|known| known == scope))
            .collect::<HashSet<_>>();
        prop_assert_eq!(&reported, &contributing(&plan));

        // A set comparison alone would pass if stacking dropped a contributing
        // scope's layers, so pin the count against the vocabulary too.
        let layers = outcome.into_result().map_err(fail("resolve layers"))?;
        prop_assert_eq!(layers.len(), expected(&root, &plan)?.0.len());
    }

    /// One file reached under two spellings contributes one layer.
    #[test]
    fn a_file_reached_twice_contributes_one_layer(plan in any_aliased_plan()) {
        let (_temp, root) = staged(&plan)?;
        // The aliased root must really yield a second candidate, or this
        // property would pass without ever exercising de-duplication.
        let spellings = builder(&root, &plan)
            .build()
            .candidates()
            .iter()
            .filter(|path| path.ends_with("project.toml"))
            .count();
        prop_assert_eq!(spellings, 2, "expected two spellings of one project file");

        let layers = compose(&root, &plan)?;
        let canonical = layers
            .iter()
            .map(|layer| layer.path().map(|path| path.as_std_path().to_path_buf()))
            .collect::<Vec<_>>();
        prop_assert!(canonical.iter().all(Option::is_some), "every layer is a file");
        let distinct = canonical.iter().collect::<HashSet<_>>();
        prop_assert_eq!(canonical.len(), distinct.len(), "two layers share a path");
        prop_assert_eq!(layers.len(), expected(&root, &plan)?.0.len());
    }

    /// Scope order is precedence order: reversing it reverses the winner.
    ///
    /// The last requested scope that contributes supplies the surviving `value`,
    /// so reversing the request hands it to the first.
    #[test]
    fn reversing_the_scope_order_reverses_the_winner(plan in any_plan()) {
        // Two contributing scopes are what make the reversal observable; with
        // one, the same file wins either way and the property says nothing.
        prop_assume!(contributing(&plan).len() > 1);
        let (_temp, root) = staged(&plan)?;

        let contributing_scopes = plan
            .order
            .iter()
            .copied()
            .filter(|scope| contributing(&plan).contains(scope))
            .collect::<Vec<_>>();
        let mut reversed = plan.clone();
        reversed.order = plan.order.iter().rev().copied().collect();
        let forward = winning_scope(compose(&root, &plan)?);
        let backward = winning_scope(compose(&root, &reversed)?);

        prop_assert_eq!(forward, contributing_scopes.last().copied());
        prop_assert_eq!(backward, contributing_scopes.first().copied());
        prop_assert_ne!(forward, backward, "reversal must change the winner");
    }
}
