//! Property tests for summary-locale candidate planning.
//!
//! The rstest tables pin the documented examples; these properties check that
//! the ordering, deduplication, source, and discovery invariants hold for
//! arbitrary lists built from the supported locale shapes.

use super::plan::plan_same_language_candidates;
use super::*;
use crate::cli_test_support;
use crate::locale::parse_locale;
use camino::Utf8PathBuf;
use proptest::prelude::*;

/// A locale tag accepted by the planner, including a variant-bearing name.
fn locale_tag() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "en",
        "en-US",
        "en-GB",
        "en-AU",
        "fr-FR",
        "fr-CA",
        "de-DE",
        "zh-Hans",
        "ja",
        "en-backup",
        "en_backup",
    ])
    .prop_map(str::to_owned)
}

/// A list of candidate or catalogued tags.
fn locale_tags() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(locale_tag(), 0..8)
}

/// Returns the first value that parses, matching the planner's preference rule.
fn first_parseable(values: &[String]) -> Option<String> {
    values
        .iter()
        .find_map(|value| parse_locale(value).ok())
        .map(|locale| locale.to_string())
}

proptest! {
    #[test]
    fn preferences_are_unique_and_keep_the_first_parseable_value(
        requested in locale_tags(),
        metadata in locale_tags(),
    ) {
        let mut args = cli_test_support::base_args();
        args.locale = requested.clone();
        let selection =
            cli_test_support::selection_with_locales(Utf8PathBuf::from("."), Some(metadata.clone()));

        let preferences = summary_locale_preferences(
            &args.locale,
            selection.locales.as_deref().unwrap_or_default(),
        )
        .candidates;
        let locales = preferences
            .iter()
            .map(|candidate| candidate.locale.to_string())
            .collect::<Vec<_>>();

        let mut unique = locales.clone();
        unique.sort();
        unique.dedup();
        prop_assert_eq!(
            locales.len(),
            unique.len(),
            "preferences should contain no duplicate locales: {:?}",
            locales,
        );
        prop_assert!(
            locales.iter().any(|locale| locale == "en-US"),
            "en-US should always be a preference: {:?}",
            locales,
        );

        let (expected_locale, expected_source) = if requested.is_empty() {
            first_parseable(&metadata).map_or_else(
                || ("en-US".to_owned(), SummaryLocaleSource::EnUs),
                |locale| (locale, SummaryLocaleSource::Metadata),
            )
        } else {
            first_parseable(&requested).map_or_else(
                || ("en-US".to_owned(), SummaryLocaleSource::EnUs),
                |locale| (locale, SummaryLocaleSource::Requested),
            )
        };
        prop_assert_eq!(
            locales.first().map(String::as_str),
            Some(expected_locale.as_str()),
            "the first preference should follow the documented order",
        );
        prop_assert_eq!(
            preferences.first().map(|candidate| candidate.source),
            Some(expected_source),
            "the first preference should record its source",
        );
    }

    #[test]
    fn fallbacks_are_unique_sorted_and_same_language(
        preferred_tag in locale_tag(),
        discovered in locale_tags(),
    ) {
        let preferred_locale =
            parse_locale(&preferred_tag).expect("the generated tag should parse");
        let preferences = vec![SummaryLocaleCandidate {
            locale: preferred_locale.clone(),
            source: SummaryLocaleSource::Requested,
        }];

        let fallbacks = plan_same_language_candidates(&preferences, &discovered);
        let locales = fallbacks
            .iter()
            .map(|candidate| candidate.locale.to_string())
            .collect::<Vec<_>>();

        let mut unique = locales.clone();
        unique.sort();
        unique.dedup();
        prop_assert_eq!(
            locales.len(),
            unique.len(),
            "fallbacks should contain no duplicate locales: {:?}",
            locales,
        );
        let mut sorted = locales.clone();
        sorted.sort();
        prop_assert_eq!(
            locales.clone(),
            sorted,
            "fallbacks should follow sorted discovery order: {:?}",
            locales,
        );
        for candidate in &fallbacks {
            prop_assert_eq!(
                candidate.source,
                SummaryLocaleSource::SameLanguage,
                "every fallback should record the same-language source",
            );
            prop_assert_eq!(
                &candidate.locale.language,
                &preferred_locale.language,
                "a fallback must share the preferred language",
            );
            prop_assert_ne!(
                &candidate.locale,
                &preferred_locale,
                "a preference must not be repeated as a fallback",
            );
        }
    }
}
