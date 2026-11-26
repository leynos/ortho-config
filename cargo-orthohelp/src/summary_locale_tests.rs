//! Unit tests for summary-locale ordering and same-language discovery.
//!
//! The planning rules are exercised through the pure
//! [`plan_same_language_candidates`] function with plain string tables, and the
//! fallible discovery adapter is covered separately with real temporary
//! directories.

use super::plan::{
    plan_same_language_candidates, same_language_summary_locales, summary_locale_preferences,
};
use super::*;
use crate::cli_test_support;
use crate::cli_test_support::PackageRoot;
use crate::locale::parse_locale;
use camino::Utf8PathBuf;
use rstest::{fixture, rstest};

/// Injects the shared default CLI arguments as an rstest fixture.
#[fixture]
fn base_args() -> Args {
    cli_test_support::base_args()
}

/// Builds an empty package root for one test.
#[fixture]
fn package_root() -> cli_test_support::PackageRoot {
    cli_test_support::PackageRoot::new()
}

/// Builds a selection whose metadata declares `locales`.
fn selection_with_locales(
    package_root: Utf8PathBuf,
    locales: Option<Vec<String>>,
) -> PackageSelection {
    cli_test_support::selection_with_locales(package_root, locales)
}

/// Renders candidates as `locale:source` pairs for table assertions.
fn rendered(candidates: &[SummaryLocaleCandidate]) -> Vec<String> {
    candidates
        .iter()
        .map(|candidate| format!("{}:{}", candidate.locale, candidate.source.as_str()))
        .collect()
}

#[rstest]
#[case::requested_first(
    vec!["en-GB".to_owned()],
    Some(vec!["fr-FR".to_owned()]),
    vec!["en-GB:requested", "en-US:en-US"]
)]
#[case::requested_multiple_uses_first(
    vec!["fr-FR".to_owned(), "en-GB".to_owned()],
    None,
    vec!["fr-FR:requested", "en-US:en-US"]
)]
// `--all-locales` is irrelevant: metadata supplies the preference without it.
#[case::metadata_without_all_locales(
    Vec::new(),
    Some(vec!["en-GB".to_owned()]),
    vec!["en-GB:metadata", "en-US:en-US"]
)]
// Reversed order proves the first metadata entry wins, not a sorted one.
#[case::metadata_order_is_preserved(
    Vec::new(),
    Some(vec!["fr-FR".to_owned(), "en-US".to_owned()]),
    vec!["fr-FR:metadata", "en-US:en-US"]
)]
// An explicit list, even an invalid one, suppresses the metadata preference.
#[case::invalid_explicit_does_not_fall_back_to_metadata(
    vec!["not a locale!".to_owned()],
    Some(vec!["fr-FR".to_owned()]),
    vec!["en-US:en-US"]
)]
#[case::defaults_to_en_us(Vec::new(), None, vec!["en-US:en-US"])]
fn summary_locale_preferences_follow_the_documented_order(
    mut base_args: Args,
    #[case] requested: Vec<String>,
    #[case] metadata_locales: Option<Vec<String>>,
    #[case] expected: Vec<&str>,
) {
    base_args.locale = requested;
    let selection = selection_with_locales(Utf8PathBuf::from("."), metadata_locales);

    let preferences = summary_locale_preferences(
        &base_args.locale,
        selection.locales.as_deref().unwrap_or_default(),
    );

    assert_eq!(
        rendered(&preferences.candidates),
        expected,
        "summary locale preferences should follow request, metadata, then en-US",
    );
}

#[rstest]
fn duplicate_preferred_locale_collapses_to_one_candidate(mut base_args: Args) {
    base_args.locale = vec!["en-US".to_owned()];
    let selection = selection_with_locales(Utf8PathBuf::from("."), None);

    let preferences = summary_locale_preferences(
        &base_args.locale,
        selection.locales.as_deref().unwrap_or_default(),
    );

    assert_eq!(
        rendered(&preferences.candidates),
        vec!["en-US:requested"],
        "an explicitly requested en-US keeps the requested source and drops the duplicate",
    );
}

#[rstest]
fn invalid_first_locale_is_skipped_in_favour_of_the_next(mut base_args: Args) {
    base_args.locale = vec!["not a locale!".to_owned(), "en-GB".to_owned()];
    let selection = selection_with_locales(Utf8PathBuf::from("."), None);

    let preferences = summary_locale_preferences(
        &base_args.locale,
        selection.locales.as_deref().unwrap_or_default(),
    );

    assert_eq!(
        rendered(&preferences.candidates),
        vec!["en-GB:requested", "en-US:en-US"],
        "an unparsable locale should not consume the preferred slot",
    );
    assert_eq!(
        preferences
            .invalid
            .iter()
            .map(|invalid| invalid.value.as_str())
            .collect::<Vec<_>>(),
        vec!["not a locale!"],
        "the invalid value should be reported to the boundary",
    );
}

/// Builds a candidate list containing one `en-US` preference.
fn en_us_preferences() -> Vec<SummaryLocaleCandidate> {
    vec![SummaryLocaleCandidate {
        locale: parse_locale("en-US").expect("parse en-US"),
        source: SummaryLocaleSource::EnUs,
    }]
}

/// Creates the listed locale directories and returns the same-language
/// fallbacks discovered for `preferences`.
fn fallbacks_for(
    package_root: &cli_test_support::PackageRoot,
    directories: &[&str],
    preferences: &[SummaryLocaleCandidate],
) -> Vec<SummaryLocaleCandidate> {
    for locale in directories {
        package_root.add_locale_dir(locale);
    }
    same_language_summary_locales(&package_root.path, preferences)
        .expect("discovery should succeed on a temporary package root")
}

#[rstest]
#[case::sorted_and_excluding_the_preference(
    &["en-GB", "en-AU", "fr-FR", "de-DE"],
    vec!["en-AU:same-language", "en-GB:same-language"],
    "fallbacks should be sorted, same-language only, and exclude the preference itself"
)]
#[case::variant_directories_rejected(
    &["en-GB", "en_backup", "en-backup", "en-Latn"],
    vec!["en-GB:same-language", "en-Latn:same-language"],
    "script-bearing locales stay eligible, while variant directories are rejected"
)]
fn same_language_fallbacks_follow_discovery_rules(
    package_root: PackageRoot,
    #[case] directories: &[&str],
    #[case] expected: Vec<&str>,
    #[case] message: &str,
) {
    let preferences = en_us_preferences();

    let fallbacks = fallbacks_for(&package_root, directories, &preferences);

    assert_eq!(rendered(&fallbacks), expected, "{message}");
}

#[rstest]
fn same_language_fallbacks_do_not_repeat_a_requested_locale(package_root: PackageRoot) {
    let preferences = vec![SummaryLocaleCandidate {
        locale: parse_locale("en-GB").expect("parse en-GB"),
        source: SummaryLocaleSource::Requested,
    }]
    .into_iter()
    .chain(en_us_preferences())
    .collect::<Vec<_>>();

    let fallbacks = fallbacks_for(&package_root, &["en-GB", "en-US"], &preferences);

    assert!(
        fallbacks.is_empty(),
        "both discovery entries are already preferences, got {:?}",
        rendered(&fallbacks),
    );
}

#[rstest]
#[case::sorted_and_unique(
    &["fr-FR", "en-GB", "en-AU", "en-GB"],
    vec!["en-AU:same-language", "en-GB:same-language"]
)]
#[case::variants_rejected_and_scripts_kept(
    &["en-backup", "zh-Hans", "en-GB"],
    vec!["en-GB:same-language"]
)]
#[case::preferences_excluded(&["en-US", "en-GB"], vec!["en-GB:same-language"])]
fn pure_planner_orders_and_filters_discovered_names(
    #[case] discovered: &[&str],
    #[case] expected: Vec<&str>,
) {
    let preferences = en_us_preferences();
    let names = discovered
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();

    let fallbacks = plan_same_language_candidates(&preferences, &names);

    assert_eq!(
        rendered(&fallbacks),
        expected,
        "the pure planner should match the documented discovery rules",
    );
}

#[rstest]
fn pure_planner_uses_preferred_order_before_sorted_names() {
    let preferences = vec![
        SummaryLocaleCandidate {
            locale: parse_locale("fr-CA").expect("parse fr-CA"),
            source: SummaryLocaleSource::Requested,
        },
        SummaryLocaleCandidate {
            locale: parse_locale("en-US").expect("parse en-US"),
            source: SummaryLocaleSource::EnUs,
        },
    ];
    let discovered = vec!["en-GB".to_owned(), "fr-FR".to_owned(), "fr-BE".to_owned()];

    let fallbacks = plan_same_language_candidates(&preferences, &discovered);

    assert_eq!(
        rendered(&fallbacks),
        vec![
            "fr-BE:same-language",
            "fr-FR:same-language",
            "en-GB:same-language"
        ],
        "each preference should contribute its matches in sorted order",
    );
}
