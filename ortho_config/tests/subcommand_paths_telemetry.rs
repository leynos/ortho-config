//! Telemetry contracts for subcommand configuration-file discovery.
//!
//! Discovery decisions are driven by paths, environment values, and host
//! state, none of which may reach an event. These cases pin the closed
//! vocabulary and check that no candidate path or variable value is recorded.

use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::{
    MapEnv, OrthoConfig, SubcommandFileContext, load_and_merge_subcommand_for_with_sources_at,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[path = "support/tracing_capture.rs"]
#[expect(
    dead_code,
    reason = "This suite needs only capture; the remaining helpers serve other discovery suites."
)]
mod capture_support;

use capture_support::{Captured, capture};

#[derive(Debug, Default, Deserialize, Parser, Serialize, OrthoConfig)]
#[command(name = "telemetry")]
#[ortho_config(prefix = "TELEMETRY_")]
struct TelemetrySubcommand {
    #[arg(long)]
    jobs: Option<u16>,
}

/// Closed field set for the discovery events; a path or value would add a key.
const ALLOWED_FIELDS: &[&str] = &[
    "event", "message", "stage", "outcome", "category", "source", "position",
];
/// Values that must never appear in any field of any event.
const SENSITIVE_INPUTS: &[&str] = &[
    "TELEMETRY_CMDS_TELEMETRY_JOBS",
    "secret-injected-value",
    "telemetry-secret-directory",
];

fn discovery_events(events: &[Captured]) -> impl Iterator<Item = &Captured> {
    events
        .iter()
        .filter(|event| event.field("event").starts_with("subcommand.paths"))
}

fn assert_bounded_and_redacted(events: &[Captured]) {
    let discovered = discovery_events(events).collect::<Vec<_>>();
    assert!(
        !discovered.is_empty(),
        "redaction checks require captured discovery events"
    );

    for event in discovered {
        for (name, value) in event.fields() {
            assert!(
                ALLOWED_FIELDS.contains(&name),
                "unexpected discovery event field `{name}`"
            );
            for sensitive in SENSITIVE_INPUTS {
                assert!(
                    !value.contains(sensitive),
                    "field `{name}` leaked `{sensitive}`: {value}"
                );
            }
        }
    }
}

/// A successful load reports a started and a finished candidate search.
#[test]
fn candidate_search_reports_a_bounded_terminal_outcome() {
    let root = tempfile::tempdir().expect("a temporary directory should be creatable");
    let cap =
        Dir::open_ambient_dir(root.path(), ambient_authority()).expect("the temp dir should open");
    cap.write("telemetry.toml", b"jobs = 3\n")
        .expect("the fixture should be written");

    let events = capture(|| {
        let merge_source = Arc::new(MapEnv::new());
        let result = load_and_merge_subcommand_for_with_sources_at(
            &TelemetrySubcommand::default(),
            SubcommandFileContext::new(root.path(), &MapEnv::new()),
            merge_source,
        );
        assert!(result.is_ok(), "subcommand load should succeed: {result:?}");
    });

    let finished = discovery_events(&events)
        .filter(|event| event.field("event") == "subcommand.paths.candidates")
        .filter(|event| !event.field("outcome").is_empty())
        .collect::<Vec<_>>();
    assert!(
        !finished.is_empty(),
        "a completed candidate search must be recorded"
    );
    assert!(
        finished
            .iter()
            .all(|event| event.field("category") == "none"),
        "a successful search must record the no-error category"
    );

    assert_bounded_and_redacted(&events);
}

/// A candidate that cannot be probed records the closed probe category.
///
/// Privileged users bypass permission bits, so the probe cannot fail for them;
/// the test detects that and returns early rather than asserting a false
/// negative.
#[cfg(unix)]
#[test]
fn failed_probe_records_the_probe_category() {
    use cap_std::fs::{Permissions, PermissionsExt};
    use std::path::Path;

    let root = tempfile::tempdir().expect("a temporary directory should be creatable");
    let cap =
        Dir::open_ambient_dir(root.path(), ambient_authority()).expect("the temp dir should open");
    // The capability handle addresses the tree relative to the directory it was
    // opened on, while `XDG_CONFIG_HOME` must carry the absolute path a real
    // process would set.
    let locked_relative = Path::new("telemetry-secret-directory");
    let locked = root.path().join(locked_relative);
    cap.create_dir_all(locked_relative.join("telemetry"))
        .expect("the locked tree should be creatable");
    cap.set_permissions(locked_relative, Permissions::from_mode(0o000))
        .expect("the XDG directory should be lockable");
    let is_privileged = cap.metadata(locked_relative.join("telemetry")).is_ok();

    let source = MapEnv::new().with_var("XDG_CONFIG_HOME", &locked);
    let probe_events = (!is_privileged).then(|| {
        capture(|| {
            let result = load_and_merge_subcommand_for_with_sources_at(
                &TelemetrySubcommand::default(),
                SubcommandFileContext::new(root.path(), &source),
                Arc::new(MapEnv::new()),
            );
            assert!(result.is_err(), "a failed probe must surface as an error");
        })
    });
    cap.set_permissions(locked_relative, Permissions::from_mode(0o700))
        .expect("the XDG directory should be unlockable");
    let Some(events) = probe_events else {
        return;
    };

    let probe_failure = discovery_events(&events)
        .find(|event| event.field("outcome") == "probe_failed")
        .expect("a failed probe must be recorded");
    assert_eq!(probe_failure.field("category"), "probe");

    assert_bounded_and_redacted(&events);
}

/// The recorded events never disclose a candidate path.
#[test]
fn discovery_events_disclose_no_paths() {
    let root = tempfile::tempdir().expect("a temporary directory should be creatable");
    let base = root.path().join("telemetry-secret-base");
    let cap =
        Dir::open_ambient_dir(root.path(), ambient_authority()).expect("the temp dir should open");
    cap.write("telemetry.toml", b"jobs = 3\n")
        .expect("the fixture should be written");

    let events = capture(|| {
        let result = load_and_merge_subcommand_for_with_sources_at(
            &TelemetrySubcommand::default(),
            SubcommandFileContext::new(&base, &MapEnv::new()),
            Arc::new(MapEnv::new()),
        );
        assert!(result.is_ok(), "subcommand load should succeed: {result:?}");
    });

    let recorded: Vec<&Captured> = discovery_events(&events).collect();
    assert!(
        !recorded.is_empty(),
        "the load must record at least one discovery event"
    );
    for event in recorded {
        for (name, value) in event.fields() {
            assert!(
                !value.contains("telemetry-secret-base"),
                "field `{name}` disclosed the candidate base: {value}"
            );
            assert!(
                !value.contains(root.path().to_string_lossy().as_ref()),
                "field `{name}` disclosed the temporary root: {value}"
            );
        }
    }
}
