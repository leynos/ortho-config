//! Identifier derivation helpers for command-line localisation.
//!
//! The helpers in this module turn clap command paths and metadata suffixes
//! into Fluent message identifiers. They are intentionally strict because an
//! invalid identifier is a command declaration bug, not a runtime locale issue.

/// Builds the Fluent identifier for a command path and suffix.
///
/// `command_path[0]` is the root segment, usually a binary name or a
/// `LocalizeCmd::with_base` override. Later elements are command-name
/// segments. `suffix` is the leaf token, such as `"about"` or
/// `"args.config.help"`. Each segment is normalized independently and joined
/// with `-`.
///
/// # Panics
///
/// Panics if a segment contains a character outside `[A-Za-z0-9_-]`, if a
/// segment is empty, or if the final identifier does not start with an ASCII
/// letter. These are programmer errors in the command declaration.
///
/// # Examples
///
/// ```rust
/// use ortho_config::message_id_for;
///
/// let id = message_id_for(&["hello_world", "cli", "greet"], "args.name.help");
///
/// assert_eq!(id, "hello_world-cli-greet-args-name-help");
/// ```
#[must_use]
pub fn message_id_for(command_path: &[impl AsRef<str>], suffix: &str) -> String {
    assert!(
        !command_path.is_empty(),
        "Fluent identifier must start with an ASCII letter: missing command root"
    );

    let suffix_segments = suffix.split('.');
    let mut segments = Vec::with_capacity(command_path.len() + suffix_segments.clone().count());
    segments.extend(command_path.iter().map(AsRef::as_ref));
    segments.extend(suffix_segments);

    let id = segments
        .into_iter()
        .map(normalize_segment)
        .collect::<Vec<_>>()
        .join("-");

    if !id
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
    {
        panic!("Fluent identifier must start with an ASCII letter: {id:?}");
    }

    id
}

pub(crate) fn normalize_segment(raw: &str) -> String {
    assert!(
        !raw.is_empty(),
        "invalid Fluent identifier segment: segment must not be empty"
    );

    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch.to_ascii_lowercase()
            } else {
                panic!("invalid Fluent identifier segment: {raw:?}");
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Tests for strict Fluent identifier derivation.

    use super::*;
    use crate::localizer::fluent;
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
            let once = normalize_segment(&segment);
            let twice = normalize_segment(&once);

            prop_assert_eq!(once, twice);
        }

        #[test]
        fn normalize_segment_exposes_case_only_collisions(segment in "[A-Za-z]{1,12}") {
            let upper = segment.to_ascii_uppercase();
            let lower = segment.to_ascii_lowercase();

            prop_assert_eq!(normalize_segment(&upper), normalize_segment(&lower));
        }
    }
}
