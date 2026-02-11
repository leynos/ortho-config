//! Unit tests for error classification and aggregation behaviour.

use clap::{Command, error::ErrorKind};
use rstest::rstest;
use std::sync::Arc;

use super::{OrthoError, is_display_request};

fn build_error(kind: ErrorKind) -> clap::Error {
    Command::new("demo").error(kind, "demo output")
}

#[rstest]
#[case(ErrorKind::DisplayHelp)]
#[case(ErrorKind::DisplayVersion)]
fn recognises_display_requests(#[case] kind: ErrorKind) {
    let err = build_error(kind);
    assert!(is_display_request(&err));
}

#[rstest]
#[case(ErrorKind::UnknownArgument)]
#[case(ErrorKind::InvalidValue)]
fn rejects_regular_errors(#[case] kind: ErrorKind) {
    let err = build_error(kind);
    assert!(!is_display_request(&err));
}

fn run_aggregate_tests<F>(name: &str, runner: F)
where
    F: Fn(Vec<Arc<OrthoError>>) -> OrthoError,
{
    assert_single_owned(name, &runner);
    assert_single_shared(name, &runner);
    assert_multi_entry(name, &runner);
}

fn assert_single_owned<F>(name: &str, runner: &F)
where
    F: Fn(Vec<Arc<OrthoError>>) -> OrthoError,
{
    let err = Arc::new(OrthoError::Validation {
        key: "k".into(),
        message: "m".into(),
    });
    let outcome = runner(vec![err]);
    assert!(
        matches!(outcome, OrthoError::Validation { .. }),
        "{name}: expected Validation, got {outcome:?}"
    );
}

fn assert_single_shared<F>(name: &str, runner: &F)
where
    F: Fn(Vec<Arc<OrthoError>>) -> OrthoError,
{
    let shared = OrthoError::gathering_arc(figment::Error::from("boom"));
    let outcome = runner(vec![Arc::clone(&shared)]);
    match outcome {
        OrthoError::Aggregate(aggregate) => {
            assert_eq!(
                aggregate.len(),
                1,
                "{name}: expected single aggregate entry"
            );
        }
        other => panic!("{name}: expected Aggregate, got {other:?}"),
    }
}

fn assert_multi_entry<F>(name: &str, runner: &F)
where
    F: Fn(Vec<Arc<OrthoError>>) -> OrthoError,
{
    let first = OrthoError::gathering_arc(figment::Error::from("one"));
    let second = OrthoError::gathering_arc(figment::Error::from("two"));
    match runner(vec![first, second]) {
        OrthoError::Aggregate(aggregate) => {
            let errors = aggregate.as_ref();
            assert_eq!(errors.len(), 2, "{name}: expected two aggregate entries");
            let borrowed: Vec<_> = errors.iter().collect();
            assert_eq!(borrowed.len(), 2, "{name}: borrowed iteration failed");
            let display = errors.to_string();
            let owned: Vec<_> = aggregate.into_iter().collect();
            assert_eq!(owned.len(), 2, "{name}: owned iteration failed");
            assert!(display.starts_with("1:"), "{name}: first entry missing");
            assert!(display.contains("\n2:"), "{name}: second entry missing");
        }
        other => panic!("{name}: expected Aggregate, got {other:?}"),
    }
}

#[test]
fn aggregate_panics_on_empty() {
    let empty: Vec<Arc<OrthoError>> = vec![];
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        OrthoError::aggregate(empty)
    }));
    assert!(result.is_err());
}

#[test]
fn try_aggregate_none_on_empty() {
    assert!(OrthoError::try_aggregate(Vec::<Arc<OrthoError>>::new()).is_none());
}

#[test]
fn both_aggregate_behaviours() {
    run_aggregate_tests("try_aggregate", |v| {
        OrthoError::try_aggregate(v).map_or_else(
            || panic!("expected error aggregation to yield a value"),
            |err| err,
        )
    });
    run_aggregate_tests("aggregate", OrthoError::aggregate);
}
