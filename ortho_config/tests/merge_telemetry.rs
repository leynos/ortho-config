//! Telemetry contracts for process-backed and injected environment merging.
//!
//! The merge layer handles environment data that must never reach an event.
//! These cases pin its small, closed telemetry vocabulary and exercise every
//! source-aware entry point that makes a terminal loading decision.

use cap_std::{ambient_authority, fs::Dir};
use figment::{Jail, Provider};
use ortho_config::{
    CsvEnv, MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource,
    load_and_merge_subcommand_with_sources, subcommand::Prefix,
};
use std::{io::Write, process::Command, sync::Arc};

const PROCESS_PROBE_MARKER: &str = "ORTHO_TELEMETRY_PROCESS_PROBE_OK";
const PROCESS_INPUT_SENTINEL: &str = "27183";

#[path = "support/tracing_capture.rs"]
#[expect(
    dead_code,
    reason = "This standalone suite needs only capture; other shared helpers serve discovery suites."
)]
mod capture_support;

use capture_support::capture;

#[path = "support/merge_telemetry_fixtures.rs"]
mod merge_telemetry_fixtures;

use merge_telemetry_fixtures::{DerivedConfig, ProfileTelemetryConfig, TelemetrySubcommand};

#[path = "support/merge_telemetry_assertions.rs"]
mod merge_telemetry_assertions;

use merge_telemetry_assertions::{assert_bounded_and_redacted, find_event};

#[test]
fn csv_env_reports_process_and_injected_success_without_input_data() {
    let executable = std::env::current_exe().expect("locate the integration-test executable");
    let output = Command::new(executable)
        .args([
            "--ignored",
            "--exact",
            "--show-output",
            "process_csv_env_success_probe",
        ])
        .env_clear()
        .env("MERGE_TELEMETRY_JOBS", PROCESS_INPUT_SENTINEL)
        .output()
        .expect("run the process-backed telemetry probe");
    let child_stdout = String::from_utf8_lossy(&output.stdout);
    let child_stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{child_stdout}\n{child_stderr}");
    assert!(
        child_stdout.contains(PROCESS_PROBE_MARKER),
        "process-backed telemetry probe did not execute: {child_stdout}"
    );

    let injected = Arc::new(
        MapEnv::new()
            .with_var("MERGE_TELEMETRY_JOBS", PROCESS_INPUT_SENTINEL)
            .with_var("UNRELATED_SECRET_KEY", "secret-injected-value")
            .with_var("UNRELATED_PATH", "/secret/injected/path"),
    );
    let injected_events = capture(|| {
        let result = CsvEnv::prefixed("MERGE_TELEMETRY_")
            .with_source(injected)
            .data();
        assert!(result.is_ok(), "injected provider should load: {result:?}");
    });
    let injected_success = find_event(&injected_events, "csv_env", "injected", "success");
    assert_eq!(injected_success.field("category"), "none");
    assert_bounded_and_redacted(&injected_events);
}

/// Assert the real process provider's event in a separately scoped process.
#[test]
#[ignore = "run by the process-backed telemetry contract"]
fn process_csv_env_success_probe() {
    let events = capture(|| {
        let result = CsvEnv::prefixed("MERGE_TELEMETRY_").data();
        assert!(result.is_ok(), "process provider should load: {result:?}");
    });
    let success = find_event(&events, "csv_env", "process", "success");
    assert_eq!(success.field("category"), "none");
    assert_bounded_and_redacted(&events);
    writeln!(std::io::stdout().lock(), "{PROCESS_PROBE_MARKER}")
        .expect("write process-backed telemetry probe marker");
}

#[test]
fn opaque_injected_transform_emits_a_bounded_failure() {
    let events = capture(|| {
        let result = CsvEnv::raw()
            .map(|key| key.into())
            .with_source(Arc::new(
                MapEnv::new().with_var("UNRELATED_SECRET_KEY", "secret-injected-value"),
            ))
            .data();
        assert!(result.is_err(), "opaque injected transform must fail");
    });

    let failure = find_event(&events, "csv_env", "injected", "failure");
    assert_eq!(failure.field("category"), "opaque_key_transform");
    assert_bounded_and_redacted(&events);
}

#[test]
fn source_aware_derived_load_reports_success_and_failure() {
    let source = Arc::new(
        MapEnv::new()
            .with_var("MERGE_TELEMETRY_JOBS", "7")
            .with_var("UNRELATED_SECRET_KEY", "secret-injected-value")
            .with_var("UNRELATED_PATH", "/secret/injected/path"),
    );
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    let success_events = capture(|| {
        let result = DerivedConfig::load_from_iter_with_sources(["telemetry"], discovery, merge);
        assert!(
            result.is_ok(),
            "derived injected load should succeed: {result:?}"
        );
    });
    let success = find_event(&success_events, "derived_load", "injected", "success");
    assert_eq!(success.field("category"), "none");
    assert_bounded_and_redacted(&success_events);

    let failing_source =
        Arc::new(MapEnv::new().with_var("MERGE_TELEMETRY_JOBS", "secret-injected-value"));
    let failing_discovery: SharedEnvSource = failing_source.clone();
    let failing_merge: SharedScanEnvSource = failing_source;
    let failure_events = capture(|| {
        let result = DerivedConfig::load_from_iter_with_sources(
            ["telemetry"],
            failing_discovery,
            failing_merge,
        );
        assert!(
            result.is_err(),
            "invalid injected value must fail derived loading"
        );
    });
    let failure = find_event(&failure_events, "derived_load", "injected", "failure");
    assert_eq!(failure.field("category"), "merge");
    assert_bounded_and_redacted(&failure_events);
}

#[test]
fn profile_load_reports_success_and_failure() {
    // The default `jobs` of 3 is present in the composed layers, so the
    // success case needs no file or environment input at all.
    let success_events = capture(|| {
        let result = ProfileTelemetryConfig::load_with_profile_from_iter(["telemetry"]);
        assert!(
            result.is_ok(),
            "profile-aware load should succeed: {result:?}"
        );
    });
    // The profile-aware entry point takes no injected source, so its events
    // are labelled `process`; only the source-aware entry points are injected.
    let success = find_event(&success_events, "profile_load", "process", "success");
    assert_eq!(success.field("category"), "none");
    assert_bounded_and_redacted(&success_events);

    // An unknown profile is a selection failure, so it must reduce to the
    // profile category without leaking the selector's value.
    let failure_events = capture(|| {
        let result =
            ProfileTelemetryConfig::load_with_profile_from_iter(["telemetry", "--profile", "nope"]);
        assert!(
            result.is_err(),
            "an unknown profile must fail the profile-aware load"
        );
    });
    let failure = find_event(&failure_events, "profile_load", "process", "failure");
    assert_eq!(failure.field("category"), "profile");
    assert_bounded_and_redacted(&failure_events);
}

/// An opted-in struct resolves a profile on the ordinary `load_from_iter`
/// boundary too, so that boundary must report the same bounded events.
///
/// This is a separate case because `find_event` requires exactly one match: a
/// second `profile_load`/`process`/`success` event in the capture above would
/// make that helper panic rather than assert.
#[test]
fn ordinary_profile_enabled_load_reports_the_profile_operation() {
    let success_events = capture(|| {
        let result = <ProfileTelemetryConfig as OrthoConfig>::load_from_iter(["telemetry"]);
        assert!(
            result.is_ok(),
            "ordinary profile-enabled load should succeed: {result:?}"
        );
    });
    let success = find_event(&success_events, "profile_load", "process", "success");
    assert_eq!(success.field("category"), "none");
    assert_bounded_and_redacted(&success_events);

    // An unknown profile fails selection on the ordinary boundary as well.
    let failure_events = capture(|| {
        let result = <ProfileTelemetryConfig as OrthoConfig>::load_from_iter([
            "telemetry",
            "--profile",
            "nope",
        ]);
        assert!(
            result.is_err(),
            "an unknown profile must fail the ordinary load"
        );
    });
    let failure = find_event(&failure_events, "profile_load", "process", "failure");
    assert_eq!(failure.field("category"), "profile");
    assert_bounded_and_redacted(&failure_events);
}

/// The injected profile entry point reports its own source label, and reports
/// the selection it read from the injected source rather than the process.
#[test]
fn injected_profile_load_reports_the_injected_source() {
    let events = capture(|| {
        let source = Arc::new(MapEnv::new().with_var("MERGE_TELEMETRY_JOBS", "7"));
        let discovery: SharedEnvSource = source.clone();
        let merge: SharedScanEnvSource = source;
        let result = ProfileTelemetryConfig::load_with_profile_from_iter_with_sources(
            ["telemetry"],
            discovery,
            merge,
        );
        let outcome = result.expect("injected profile load should succeed");
        assert!(outcome.selection().is_empty(), "no profile was selected");
        assert_eq!(outcome.config().jobs, 7, "the injected value must win");
    });
    let success = find_event(&events, "profile_load", "injected", "success");
    assert_eq!(success.field("category"), "none");
    assert_bounded_and_redacted(&events);
}

/// A command-line parse failure is not a profile-selection failure, so it must
/// reduce to the command-line category rather than the profile one.
///
/// This is a separate case because `find_event` requires exactly one match: the
/// unknown-profile cases above emit a `profile_load`/`process`/`failure` event
/// of their own, and two in one capture would make that helper panic.
#[test]
fn ordinary_profile_enabled_load_reports_a_parse_failure() {
    let events = capture(|| {
        let result =
            <ProfileTelemetryConfig as OrthoConfig>::load_from_iter(["telemetry", "--bogus"]);
        assert!(
            result.is_err(),
            "an unknown argument must fail the ordinary load"
        );
    });

    let failure = find_event(&events, "profile_load", "process", "failure");
    assert_eq!(failure.field("category"), "cli");
    assert_bounded_and_redacted(&events);
}

/// A reserved key inside a profile body is a profile-extraction failure, so it
/// must reduce to the profile category on the injected boundary.
///
/// `cmds` is rejected by `validate_profile_body` (decision D11) because
/// subcommand loading ignores profiles, so no configuration is silently dead.
#[test]
fn injected_profile_load_reports_a_forbidden_key_failure() {
    let fixture_dir = tempfile::tempdir().expect("create forbidden-key fixture directory");
    let cap = Dir::open_ambient_dir(fixture_dir.path(), ambient_authority())
        .expect("open forbidden-key fixture directory");
    cap.write("forbidden.toml", b"[profile.ci]\ncmds = {}\n")
        .expect("write forbidden-key fixture");
    let config_path = fixture_dir.path().join("forbidden.toml");

    let events = capture(|| {
        let source = Arc::new(
            MapEnv::new()
                .with_var("MERGE_TELEMETRY_CONFIG_PATH", &config_path)
                .with_var("MERGE_TELEMETRY_PROFILE", "ci"),
        );
        let discovery: SharedEnvSource = source.clone();
        let merge: SharedScanEnvSource = source;
        let result = ProfileTelemetryConfig::load_with_profile_from_iter_with_sources(
            ["telemetry"],
            discovery,
            merge,
        );
        assert!(
            result.is_err(),
            "a reserved profile-body key must fail the injected load"
        );
    });

    let failure = find_event(&events, "profile_load", "injected", "failure");
    assert_eq!(failure.field("category"), "profile");
    assert_bounded_and_redacted(&events);
}

/// The ordinary injected boundary reports the profile operation on an opted-in
/// struct.
///
/// `load_from_iter_with_sources` serves both kinds of struct: a legacy one
/// merely loads, while an opted-in one resolves a profile selection first. So
/// an opted-in struct must report `profile_load` here rather than the legacy
/// `derived_load` label, which would misattribute the load and contradict the
/// same struct's process-backed twin.
#[test]
fn ordinary_injected_profile_load_reports_the_profile_operation() {
    let success_events = capture(|| {
        let source = Arc::new(MapEnv::new().with_var("MERGE_TELEMETRY_JOBS", "7"));
        let discovery: SharedEnvSource = source.clone();
        let merge: SharedScanEnvSource = source;
        let result = <ProfileTelemetryConfig as OrthoConfig>::load_from_iter_with_sources(
            ["telemetry"],
            discovery,
            merge,
        );
        let config = result.expect("ordinary injected load should succeed");
        assert_eq!(config.jobs, 7, "the injected value must win");
    });
    let success = find_event(&success_events, "profile_load", "injected", "success");
    assert_eq!(success.field("category"), "none");
    assert_bounded_and_redacted(&success_events);

    // An unknown profile fails selection on this boundary as well, and the
    // injected selector is what selects it.
    let failure_events = capture(|| {
        let source = Arc::new(MapEnv::new().with_var("MERGE_TELEMETRY_PROFILE", "nope"));
        let discovery: SharedEnvSource = source.clone();
        let merge: SharedScanEnvSource = source;
        let result = <ProfileTelemetryConfig as OrthoConfig>::load_from_iter_with_sources(
            ["telemetry"],
            discovery,
            merge,
        );
        assert!(
            result.is_err(),
            "an unknown profile must fail the ordinary injected load"
        );
    });
    let failure = find_event(&failure_events, "profile_load", "injected", "failure");
    assert_eq!(failure.field("category"), "profile");
    assert_bounded_and_redacted(&failure_events);
}

#[test]
fn source_aware_subcommand_load_reports_success_and_failure() {
    let success_events = capture(|| {
        let source = Arc::new(MapEnv::new().with_var("MERGE_CMDS_TELEMETRY_JOBS", "7"));
        let result = load_and_merge_subcommand_with_sources(
            &Prefix::new("MERGE"),
            &TelemetrySubcommand::default(),
            source,
        );
        assert!(
            result.is_ok(),
            "subcommand injected load should succeed: {result:?}"
        );
    });
    let success = find_event(&success_events, "subcommand_load", "injected", "success");
    assert_eq!(success.field("category"), "none");
    assert_bounded_and_redacted(&success_events);

    let failure_events = capture(|| {
        let source =
            Arc::new(MapEnv::new().with_var("MERGE_CMDS_TELEMETRY_JOBS", "secret-injected-value"));
        let result = load_and_merge_subcommand_with_sources(
            &Prefix::new("MERGE"),
            &TelemetrySubcommand::default(),
            source,
        );
        assert!(
            result.is_err(),
            "invalid injected value must fail subcommand loading"
        );
    });
    let failure = find_event(&failure_events, "subcommand_load", "injected", "failure");
    assert_eq!(failure.field("category"), "merge");
    assert_bounded_and_redacted(&failure_events);
}
