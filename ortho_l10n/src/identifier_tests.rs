//! Tests for strict Fluent identifier derivation.

use super::*;
use crate::fluent;
use proptest::prelude::*;
use rstest::rstest;

#[rstest]
#[case::base_only(["hello_world", "cli"], "about", "hello_world-cli-about")]
#[case::nested_command(
        ["hello_world", "cli", "greet"],
        "long_about",
        "hello_world-cli-greet-long_about"
    )]
#[case::argument_help(
        ["hello_world", "cli", "take-leave"],
        "args.reason.help",
        "hello_world-cli-take-leave-args-reason-help"
    )]
#[case::numeric_middle_segment(
        ["hello_world", "cli", "2026"],
        "usage",
        "hello_world-cli-2026-usage"
    )]
fn message_id_for_derives_expected_identifier<const N: usize>(
    #[case] command_path: [&str; N],
    #[case] suffix: &str,
    #[case] expected: &str,
) {
    assert_eq!(message_id_for(&command_path, suffix), expected);
}

#[rstest]
fn message_id_for_matches_loaded_catalogue_normalization() {
    let runtime_id = message_id_for(&["hello_world", "cli"], "about");
    let loaded = fluent::normalize_resource_ids("hello_world.cli.about = About text");

    assert_eq!(loaded, format!("{runtime_id} = About text"));
}

#[rstest]
#[case::space(["hello world"], "about")]
#[case::non_ascii(["ключ"], "about")]
#[case::dot_inside_segment(["hello.world"], "about")]
#[case::consecutive_dots(["hello_world", "cli"], "args..help")]
#[case::slash_in_suffix(["hello_world", "cli"], "args.config/path.help")]
#[should_panic(expected = "invalid Fluent identifier segment")]
fn message_id_for_rejects_unrepresentable_segments<const N: usize>(
    #[case] command_path: [&str; N],
    #[case] suffix: &str,
) {
    drop(message_id_for(&command_path, suffix));
}

#[rstest]
#[case::empty_path([], "about")]
#[case::numeric_root(["123"], "about")]
#[should_panic(expected = "Fluent identifier must start with an ASCII letter")]
fn message_id_for_rejects_ids_without_leading_letter<const N: usize>(
    #[case] command_path: [&str; N],
    #[case] suffix: &str,
) {
    drop(message_id_for(&command_path, suffix));
}

prop_compose! {
    fn fluent_root_segment()(
        first in "[A-Za-z]",
        rest in "[A-Za-z0-9_-]{0,12}",
    ) -> String {
        format!("{first}{rest}")
    }
}

prop_compose! {
    fn fluent_tail_segment()(segment in "[A-Za-z0-9_-]{1,12}") -> String {
        segment
    }
}

prop_compose! {
    fn fluent_command_path()(
        root in fluent_root_segment(),
        tail in proptest::collection::vec(fluent_tail_segment(), 0..4),
    ) -> Vec<String> {
        let mut path = Vec::with_capacity(tail.len() + 1);
        path.push(root);
        path.extend(tail);
        path
    }
}

prop_compose! {
    fn fluent_suffix()(
        first in fluent_tail_segment(),
        rest in proptest::collection::vec(fluent_tail_segment(), 0..4),
    ) -> String {
        let mut segments = Vec::with_capacity(rest.len() + 1);
        segments.push(first);
        segments.extend(rest);
        segments.join(".")
    }
}

proptest! {
    #[test]
    fn message_id_for_outputs_valid_fluent_identifier(
        command_path in fluent_command_path(),
        suffix in fluent_suffix(),
    ) {
        let id = message_id_for(&command_path, &suffix);

        prop_assert!(
            id.chars().next().is_some_and(|first| first.is_ascii_alphabetic()),
            "id should start with an ASCII letter: {id}"
        );
        prop_assert!(
            id.chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')),
            "id should contain only Fluent identifier characters: {id}"
        );
    }

    #[test]
    fn normalize_segment_is_idempotent(segment in fluent_tail_segment()) {
        let once = try_normalize_segment(&segment).expect("valid generated segment");
        let twice = try_normalize_segment(&once).expect("normalized segment");

        prop_assert_eq!(once, twice);
    }

    #[test]
    fn normalize_segment_exposes_case_only_collisions(segment in "[A-Za-z]{1,12}") {
        let upper = segment.to_ascii_uppercase();
        let lower = segment.to_ascii_lowercase();

        prop_assert_eq!(try_normalize_segment(&upper), try_normalize_segment(&lower));
    }
}

/// Structured errors retain the offending input and the legacy panic text.
#[rstest]
#[case::missing(
    vec![], "bad/name",
    IdentifierError::EmptyPath,
    "Fluent identifier must start with an ASCII letter: missing command root",
)]
#[case::empty_segment(
    vec!["demo"], "args..help",
    IdentifierError::EmptySegment { input: String::new() },
    "invalid Fluent identifier segment: segment must not be empty",
)]
#[case::invalid_character(
    vec!["demo"], "bad/name",
    IdentifierError::InvalidCharacter { input: "bad/name".to_owned() },
    "invalid Fluent identifier segment: \"bad/name\"",
)]
#[case::numeric_root(
    vec!["123"], "help",
    IdentifierError::InvalidLeadingCharacter { input: "123-help".to_owned() },
    "Fluent identifier must start with an ASCII letter: \"123-help\"",
)]
#[case::segment_error_precedes_root_check(
    vec!["123"], "bad/name",
    IdentifierError::InvalidCharacter { input: "bad/name".to_owned() },
    "invalid Fluent identifier segment: \"bad/name\"",
)]
fn structured_errors_and_panics_preserve_the_contract(
    #[case] path: Vec<&str>,
    #[case] suffix: &str,
    #[case] expected: IdentifierError,
    #[case] text: &str,
) {
    let error = try_message_id_for(&path, suffix).expect_err("invalid identifier");
    assert_eq!(error, expected);
    assert_eq!(error.to_string(), text);
    let panic = std::panic::catch_unwind(|| message_id_for(&path, suffix))
        .expect_err("legacy API must panic");
    let rendered = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied());
    assert_eq!(rendered, Some(text));
}
