//! Strict Fluent identifier segment normaliser for the derive macro.
//!
//! This is the test-locked macro-side twin of
//! `ortho_config::localizer::identifier::normalize_segment`. The two
//! implementations cannot share source (the macro crate is `proc-macro = true`
//! and must stay build-independent of `ortho_config`), so agreements is locked
//! by (a) this marker, (b) the dev-dependency-cycle property test, and (c) the
//! cross-crate agreement tests in `ortho_config/tests/`.
//!
//! Unlike the runtime twin, this normaliser returns spanned [`syn::Error`]s so
//! a bad identifier is a compile-time diagnostic at the offending field or
//! attribute, never a panic.
//!
//! NORMALIZATION-RULES-VERSION: 1

use proc_macro2::Span;
use syn::Error;

/// Normalises a single Fluent identifier segment.
///
/// Lowercase ASCII alphanumerics, `-`, and `_` pass through; any other
/// character, or an empty segment, is a spanned error.
pub(crate) fn normalize_segment(raw: &str, span: Span) -> Result<String, Error> {
    if raw.is_empty() {
        return Err(Error::new(
            span,
            "invalid Fluent identifier segment: segment must not be empty",
        ));
    }

    for ch in raw.chars() {
        if !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')) {
            return Err(Error::new(
                span,
                format!("invalid Fluent identifier segment: {raw:?}"),
            ));
        }
    }

    Ok(raw.to_ascii_lowercase())
}

/// Joins one or more normalised segments into a Fluent identifier.
///
/// The joined identifier must start with an ASCII letter, matching the runtime
/// convention (Fluent message ids have the same lexical rule).
pub(crate) fn join_identifier(segments: &[String], span: Span) -> Result<String, Error> {
    let joined = segments.join("-");
    let Some(first) = joined.chars().next() else {
        return Err(Error::new(
            span,
            "Fluent identifier must start with an ASCII letter: missing command root",
        ));
    };
    if !first.is_ascii_alphabetic() {
        return Err(Error::new(
            span,
            format!("Fluent identifier must start with an ASCII letter: {joined:?}"),
        ));
    }
    Ok(joined)
}

#[cfg(test)]
mod tests {
    //! Tests for the strict normaliser twin.

    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    const MARKER: &str = "NORMALIZATION-RULES-VERSION: 1";

    #[rstest]
    #[case("hello", "hello")]
    #[case("HELLO", "hello")]
    #[case("Hello_World", "hello_world")]
    #[case("hello-world", "hello-world")]
    #[case("2026", "2026")]
    #[case("a1-b2_c3", "a1-b2_c3")]
    fn normalizes_ascii_segments(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(
            normalize_segment(input, proc_macro2::Span::call_site())
                .expect("segment should normalise"),
            expected
        );
    }

    #[rstest]
    #[case("")]
    #[case("hello world")]
    #[case("héllo")]
    #[case("hello.world")]
    #[case("hello/world")]
    #[case("ключ")]
    fn rejects_unrepresentable_segments(#[case] input: &str) {
        assert!(
            normalize_segment(input, proc_macro2::Span::call_site()).is_err(),
            "segment {input:?} should be rejected"
        );
    }

    #[rstest]
    fn marker_version_is_present_in_source() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/derive/generate/localization/identifier.rs"
        ));
        assert!(
            source.contains(MARKER),
            "normaliser source must carry the {MARKER} gate"
        );
    }

    #[rstest]
    fn marker_version_matches_runtime_twin() {
        // Decision D-8 version gate: editing the normalization rules in one
        // implementation mechanically points at the other by failing here.
        let runtime = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ortho_config/src/localizer/identifier.rs"
        ));
        let ours = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/derive/generate/localization/identifier.rs"
        ));
        let runtime_version = marker_version_from(runtime)
            .expect("runtime twin must carry the marker version comment");
        let our_version =
            marker_version_from(ours).expect("normaliser must carry the marker version comment");
        assert_eq!(
            our_version, runtime_version,
            "normalization-rules version mismatch between the macro normaliser and the runtime twin"
        );
    }

    /// Extracts the `NORMALIZATION-RULES-VERSION` value from a comment-marked
    /// source (the marker lives in a `//!` module doc comment in both files).
    fn marker_version_from(source: &str) -> Option<&str> {
        source.lines().find_map(|line| {
            line.trim_start_matches("//!")
                .trim()
                .strip_prefix("NORMALIZATION-RULES-VERSION:")
                .map(str::trim)
        })
    }

    // A raw identifier segment that may start with any Fluent char.
    prop_compose! {
        fn segment()(
            raw in "[A-Za-z0-9_-]{1,12}",
        ) -> String { raw }
    }

    // A segment that starts with an ASCII letter (the runtime and the macro
    // twin reject identifiers that do not start with a letter).
    prop_compose! {
        fn root_segment()(
            first in "[A-Za-z]",
            rest in "[A-Za-z0-9_-]{0,12}",
        ) -> String {
            format!("{first}{rest}")
        }
    }

    proptest! {
        // The macro-side normaliser must agree byte-for-byte with the runtime
        // `message_id_for` (Decision D-8 dev-dependency cycle). For generated
        // command paths whose root starts with a letter, and dotted suffixes,
        // the twin's joined output equals the runtime identifier exactly.
        #[test]
        fn joined_output_agrees_with_message_id_for(
            root in root_segment(),
            tail in proptest::collection::vec(segment(), 0..3),
            suffix_segments in proptest::collection::vec(segment(), 1..4),
        ) {
            let mut path = vec![root];
            path.extend(tail);
            let suffix = suffix_segments.join(".");
            let span = proc_macro2::Span::call_site();
            let normalised_path = path
                .iter()
                .map(|s| normalize_segment(s, span).expect("generated path segment normalises"))
                .collect::<Vec<_>>();
            let normalised_tail = suffix
                .split('.')
                .map(|s| normalize_segment(s, span).expect("generated suffix segment normalises"))
                .collect::<Vec<_>>();
            let mut all = normalised_path.clone();
            all.extend(normalised_tail);
            let joined = join_identifier(&all, span).expect("generated id joins");

            let expected = ortho_config::message_id_for(&path, &suffix);

            prop_assert_eq!(joined, expected);
        }

        #[test]
        fn normalisation_is_idempotent(raw in segment()) {
            let span = proc_macro2::Span::call_site();
            let once = normalize_segment(&raw, span).expect("generated segment normalises once");
            let twice = normalize_segment(&once, span).expect("normalised segment is stable");
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn joined_output_is_a_valid_fluent_identifier(
            root in root_segment(),
            tail in proptest::collection::vec(segment(), 0..3),
            suffix_segments in proptest::collection::vec(segment(), 1..4),
        ) {
            let mut path = vec![root];
            path.extend(tail);
            let suffix = suffix_segments.join(".");
            let span = proc_macro2::Span::call_site();
            let mut segments = path
                .iter()
                .map(|s| normalize_segment(s, span).expect("generated path segment normalises"))
                .collect::<Vec<_>>();
            segments.extend(
                suffix
                    .split('.')
                    .map(|s| normalize_segment(s, span).expect("generated suffix segment normalises")),
            );
            let joined = join_identifier(&segments, span).expect("generated id joins");

            prop_assert!(
                joined.chars().next().is_some_and(|c| c.is_ascii_alphabetic()),
                "joined id must start with an ASCII letter: {joined}"
            );
            prop_assert!(
                joined.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')),
                "joined id must contain only Fluent characters: {joined}"
            );
        }
    }
}
