//! Telemetry emitted by the `ConfigFilePolicy` resolution path.
//!
//! A policy assembles no candidate list: it either resolves an explicit
//! selector or hands the work to the automatic scope walk. None of the
//! per-candidate events therefore fire on this path, so without an event of
//! its own a policy run is silent — an operator who passed an explicit path
//! would have no evidence that discovery ran at all.
//!
//! Carried in a submodule of `discovery_telemetry.rs` rather than a suite of
//! its own: these cases share that suite's capture harness, and a second
//! top-level test file would compile a second copy of it. The split is only to
//! keep each file inside the repository's 400-line limit, and it mirrors the
//! neighbouring `load_source` module.

use super::capture_support::{capture, only, write_fixture, write_fixture_with};
use ortho_config::{ConfigDiscovery, ConfigFilePolicy, ConfigPathSelector, ExplicitMode, MapEnv};
use rstest::rstest;
use std::path::PathBuf;
use std::sync::Arc;

/// A policy over a discovery that reads only `env`.
///
/// Project roots are cleared so nothing resolves relative to the working
/// directory of whichever process ran the test, and the selector chain is left
/// to the caller: that is what decides whether resolution is explicit or
/// automatic.
fn policy_with(env: MapEnv) -> ConfigFilePolicy {
    ConfigFilePolicy::from_builder(
        ConfigDiscovery::builder("demo")
            .clear_project_roots()
            .env_source(Arc::new(env)),
    )
}

/// A winning selector is reported by its constructor class, not by its label.
///
/// Both selectors are given a distinctive label and, in the environment case,
/// a distinctive variable name. Neither may appear: both are caller-supplied
/// text, and the class is the whole of what the event is allowed to say about
/// which rung won. The CLI case runs first in the chain, so the environment
/// selector is only reached when it does not — which is what makes the second
/// case non-vacuous.
#[rstest]
#[case::cli(
    ConfigPathSelector::cli(Some(PathBuf::from("selected.toml"))).label("sekrit-cli-label"),
    MapEnv::new(),
    "cli"
)]
#[case::environment(
    ConfigPathSelector::env("DEMO_CONFIG").label("sekrit-env-label"),
    MapEnv::new().with_var("DEMO_CONFIG", "/sekrit-env-path"),
    "environment"
)]
/// A selector win carries its class, which is what separates a
/// caller-supplied path from an environment rung in the event stream.
fn resolve_layers_reports_the_winning_selector_class(
    #[case] selector: ConfigPathSelector,
    #[case] env: MapEnv,
    #[case] expected: &str,
) {
    let policy = policy_with(env).selectors([selector]);

    let events = capture(|| policy.resolve_layers());
    let policy_event = only(&events, "discovery.policy");
    assert_eq!(policy_event.field("operation"), "policy_resolve");
    assert_eq!(policy_event.field("selector_class"), expected);
}

/// An *absent* selected file reports the outcome the mode gives it.
///
/// This is the case the review found completely silent. The legacy
/// `compose_layers()` path yields an attempt, a candidate, and a load outcome;
/// a policy yields nothing, so before this the only evidence an operator had
/// was the error itself. `mode` decides what an absent file *means*, and both
/// answers are drawn from the vocabulary the legacy path already uses.
///
/// Absence is not failure, which is why the optional case reads `not_found`
/// rather than `optional_failure`: the loader answers `Ok(None)` for a path
/// that is not there, and only the `required` arm turns that into an error.
/// RFC 0002's table gives `Optional` + "rung wins, missing" as "no layers,
/// stop" against `RequiredExclusive`'s "error on that path, stop", so a
/// tolerated absence and a reported one are distinct outcomes, not one outcome
/// relabelled. `optional_failure` is reserved for a path that was *found and
/// could not be read*, which the malformed case below pins.
#[rstest]
#[case::required(ExplicitMode::RequiredExclusive, "required_failure")]
#[case::optional(ExplicitMode::Optional, "not_found")]
fn an_absent_selected_file_reports_the_mode_outcome(
    #[case] mode: ExplicitMode,
    #[case] expected: &str,
) {
    let policy = policy_with(MapEnv::new())
        .selectors([ConfigPathSelector::cli(Some(PathBuf::from("missing.toml")))])
        .explicit_mode(mode);

    let events = capture(|| policy.resolve_layers());
    let load = only(&events, "discovery.load");
    assert_eq!(load.field("operation"), "policy_resolve");
    assert_eq!(load.field("outcome"), expected);
}

/// A selected file that *cannot be read* is the case that reports a failure.
///
/// This is the arm of the policy path that no test pinned: `optional_failure`
/// is emitted only when the loader returns `Err`, so a fixture that is simply
/// absent could never reach it. The contrast with the case above is the whole
/// point — the two modes agree about a malformed file and disagree about a
/// missing one, and an operator reading the event stream has to be able to tell
/// a tolerable absence from a real defect.
#[rstest]
#[case::required(ExplicitMode::RequiredExclusive, "required_failure")]
#[case::optional(ExplicitMode::Optional, "optional_failure")]
fn an_unreadable_selected_file_reports_the_mode_outcome(
    #[case] mode: ExplicitMode,
    #[case] expected: &str,
) {
    let dir = tempfile::tempdir().expect("a temporary directory should be creatable");
    let malformed = write_fixture_with(dir.path(), "malformed.toml", "value = ???\n")
        .expect("fixture should be written");
    let policy = policy_with(MapEnv::new())
        .selectors([ConfigPathSelector::cli(Some(malformed))])
        .explicit_mode(mode);

    let events = capture(|| policy.resolve_layers());
    let load = only(&events, "discovery.load");
    assert_eq!(load.field("operation"), "policy_resolve");
    assert_eq!(load.field("outcome"), expected);
}

/// The automatic branch is reported too, and names no selector class.
///
/// A policy with no selectors takes the automatic branch: the scopes resolve
/// in place of an explicit path. The resolution event still fires, with the
/// class field empty rather than one of the two classes — an absent
/// `selector_class` is what tells an operator no explicit path won, which is
/// precisely what a policy that resolved nothing must not leave them guessing
/// about. The branch then runs under the legacy operation vocabulary rather
/// than a third one of its own.
#[test]
fn resolve_layers_reports_no_class_when_no_selector_wins() {
    let policy = policy_with(MapEnv::new());

    let events = capture(|| policy.resolve_layers());
    let policy_event = only(&events, "discovery.policy");
    assert_eq!(policy_event.field("operation"), "policy_resolve");
    assert_eq!(policy_event.field("selector_class"), "");

    // The automatic half is the legacy path unchanged. Only the attempt is
    // pinned: whether a file loads depends on the host's `/etc/xdg`, so the
    // outcome is deliberately left to the cases that inject a scope base.
    assert_eq!(
        only(&events, "discovery.attempt").field("operation"),
        "compose_layers"
    );
}

/// A selector's class, label, path, and variable name stay out of every field.
///
/// The shared redaction check in the parent suite drives the legacy operations
/// only, and the policy path shares none of their plumbing — so the policy
/// resolution is exercised here with distinctive caller-supplied text and the
/// captured fields are inspected on their own. The selector's label and the
/// file name are deliberately the same string: a field carrying either would
/// then be caught by one assertion rather than needing two.
#[test]
fn the_policy_event_carries_no_caller_supplied_text() {
    const SHARED: &str = "sekrit-cli-label";

    let dir = tempfile::tempdir().expect("a temporary directory should be creatable");
    let selected =
        write_fixture(dir.path(), "sekrit-cli-label.toml").expect("fixture should be written");

    let policy = policy_with(MapEnv::new())
        .selectors([ConfigPathSelector::cli(Some(selected)).label(SHARED)]);

    let events = capture(|| policy.resolve_layers());
    assert!(
        !events.is_empty(),
        "the redaction check is vacuous unless events were captured"
    );

    for event in &events {
        for (name, value) in event.fields() {
            for leak in [SHARED, "DEMO_CONFIG"] {
                assert!(
                    !value.contains(leak),
                    "field `{name}` leaked `{leak}`: {value}"
                );
            }
            assert!(
                !value.contains(dir.path().to_string_lossy().as_ref()),
                "field `{name}` leaked the temporary directory: {value}"
            );
        }
    }
}
