//! Unit tests for agent-context summary-locale selection.
//!
//! Each case builds a private package root with its own `locales/` tree, so
//! the selection order is observed through real Fluent resources rather than
//! through a stub localizer. Distinct `demo-about` text identifies which
//! catalogue supplied the summary.

use super::*;
use crate::cli_test_support;
use crate::cli_test_support::PackageRoot;
use crate::summary_locale::SummaryLocaleSource;
use ortho_config::Localizer;
use rstest::{fixture, rstest};

const ABOUT_ID: &str = "demo-about";

/// Summary text shipped by the `en-US` catalogue in these cases.
const AMERICAN_SUMMARY: &str = "American English summary.";

/// Builds an empty package root for one test.
#[fixture]
fn package_root() -> cli_test_support::PackageRoot {
    cli_test_support::PackageRoot::new()
}

/// Builds arguments that request the agent-context format.
fn args_for(locales: &[&str]) -> Args {
    let mut args = cli_test_support::base_args();
    args.format = OutputFormat::AgentContext;
    args.locale = locales.iter().map(|locale| (*locale).to_owned()).collect();
    args
}

/// Selects the summary localizer for `args` rooted at `package_root`.
fn select(
    args: &Args,
    package_root: &Utf8PathBuf,
    metadata_locales: Option<Vec<String>>,
) -> Option<SummaryLocalizer> {
    cli_test_support::ensure_capture_subscriber();
    let selection =
        cli_test_support::selection_with_locales(package_root.clone(), metadata_locales);
    build_agent_context_localizer_if_requested(args, &selection)
}

/// Asserts the selected locale, the recorded source, and the summary text.
fn assert_selection_with_source(
    selected: Option<SummaryLocalizer>,
    expected_locale: &str,
    expected_source: SummaryLocaleSource,
    expected_summary: &str,
) {
    let resolved = selected.expect("a summary localizer should be selected");
    assert_eq!(
        resolved.locale.to_string(),
        expected_locale,
        "selected summary locale should match the expected catalogue",
    );
    assert_eq!(
        resolved.source, expected_source,
        "the recorded source should explain why this locale was chosen",
    );
    assert_eq!(
        resolved.localizer.lookup(ABOUT_ID, None).as_deref(),
        Some(expected_summary),
        "the selected localizer should resolve the package's about message",
    );
}

/// Asserts the selection's locale and summary without pinning the source.
fn assert_selection(
    selected: Option<SummaryLocalizer>,
    expected_locale: &str,
    expected_summary: &str,
) {
    let resolved = selected.expect("a summary localizer should be selected");
    assert_eq!(
        resolved.locale.to_string(),
        expected_locale,
        "selected summary locale should match the expected catalogue",
    );
    assert_eq!(
        resolved.localizer.lookup(ABOUT_ID, None).as_deref(),
        Some(expected_summary),
        "the selected localizer should resolve the package's about message",
    );
}

/// Asserts that `selected` is the `en-US` candidate with its usual summary.
///
/// Several cases share this expectation, so the assertion lives here rather
/// than being repeated with the same three literal arguments.
fn assert_en_us_selection(selected: Option<SummaryLocalizer>) {
    assert_selection_with_source(
        selected,
        "en-US",
        SummaryLocaleSource::EnUs,
        AMERICAN_SUMMARY,
    );
}

#[rstest]
#[case::requested_en_gb(&["en-GB"], None, SummaryLocaleSource::Requested)]
#[case::language_only_request(&["en"], None, SummaryLocaleSource::SameLanguage)]
#[case::metadata_without_all_locales(
    &[],
    Some(vec!["en-GB".to_owned()]),
    SummaryLocaleSource::Metadata
)]
fn en_gb_catalogue_supplies_summaries(
    package_root: PackageRoot,
    #[case] requested: &[&str],
    #[case] metadata_locales: Option<Vec<String>>,
    #[case] expected_source: SummaryLocaleSource,
) {
    package_root.add_catalogue("en-GB", "demo-about = British English summary.\n");
    let args = args_for(requested);

    let selected = select(&args, &package_root.path, metadata_locales);

    assert_selection_with_source(
        selected,
        "en-GB",
        expected_source,
        "British English summary.",
    );
}

/// `en-US` outranks a same-language sibling it was not asked for.
///
/// Reversing the preference/fallback chain would leave every other case green,
/// because the requested cases exclude their own locale from fallbacks.
#[rstest]
fn en_us_candidate_precedes_same_language_discovery(package_root: PackageRoot) {
    package_root.add_catalogue("en-US", "demo-about = American English summary.\n");
    package_root.add_catalogue("en-GB", "demo-about = British English summary.\n");
    let args = args_for(&[]);

    let selected = select(&args, &package_root.path, None);

    assert_en_us_selection(selected);
}

/// A missing preferred catalogue still yields `en-US` before `fr-FR`.
///
/// The warning's `tried` list must name only the candidates actually examined,
/// not `fr-FR`, which selection never reaches.
#[rstest]
fn en_us_candidate_precedes_same_language_fallback_for_a_missing_request(
    package_root: PackageRoot,
) {
    package_root.add_catalogue("fr-FR", "demo-about = Resume francais.\n");
    package_root.add_catalogue("en-US", "demo-about = American English summary.\n");
    let args = args_for(&["fr-CA"]);

    let mut selected = None;
    let events = cli_test_support::capture_events(|| {
        selected = select(&args, &package_root.path, None);
    });

    assert_selection_with_source(
        selected,
        "en-US",
        SummaryLocaleSource::EnUs,
        "American English summary.",
    );
    let messages = events
        .iter()
        .map(|event| event.field("message").to_owned())
        .collect::<Vec<_>>();
    let found = events
        .iter()
        .find(|event| event.message_contains("differs from the first preference"));
    let warning =
        found.unwrap_or_else(|| panic!("the en-US fall-through should warn: {messages:?}"));
    assert_eq!(
        warning.field("tried"),
        "fr-CA",
        "only the examined fr-CA candidate should be reported as tried",
    );
}

/// A same-language catalogue reports its own source when it is chosen.
#[rstest]
fn same_language_fallback_reports_its_source(package_root: PackageRoot) {
    package_root.add_catalogue("fr-FR", "demo-about = Resume francais.\n");
    let args = args_for(&["fr-CA"]);

    let selected = select(&args, &package_root.path, None);
    let resolved = selected.expect("the fr-FR fallback should build");

    assert_eq!(
        resolved.source,
        SummaryLocaleSource::SameLanguage,
        "a discovered sibling should be reported as the same-language source",
    );
    assert_eq!(
        resolved.locale.to_string(),
        "fr-FR",
        "the sibling catalogue should supply the summary",
    );
}

/// An invalid metadata entry is reported, and the next parseable one wins.
#[rstest]
fn invalid_metadata_locale_is_skipped_and_reported(package_root: PackageRoot) {
    package_root.add_catalogue("fr-FR", "demo-about = Resume francais.\n");
    let args = args_for(&[]);

    let mut selected = None;
    let events = cli_test_support::capture_events(|| {
        selected = select(
            &args,
            &package_root.path,
            Some(vec!["not a locale!".to_owned(), "fr-FR".to_owned()]),
        );
    });

    assert_selection_with_source(
        selected,
        "fr-FR",
        SummaryLocaleSource::Metadata,
        "Resume francais.",
    );
    let found = events
        .iter()
        .find(|event| event.message_contains("ignoring invalid agent-context summary locale"));
    let warning = found.expect("the invalid metadata locale should warn");
    assert_eq!(
        warning.field("locale"),
        "not a locale!",
        "the warning should name the invalid metadata value",
    );
}

/// A variant-bearing sibling sorts after a regional catalogue.
#[rstest]
fn variant_sibling_does_not_displace_a_regional_catalogue(package_root: PackageRoot) {
    package_root.add_catalogue("en-GB", "demo-about = British English summary.\n");
    package_root.add_catalogue("en-backup", "demo-about = Backup directory summary.\n");
    package_root.add_catalogue("en_backup", "demo-about = Underscore backup summary.\n");
    let args = args_for(&[]);

    let selected = select(&args, &package_root.path, None);

    assert_selection(selected, "en-GB", "British English summary.");
}

/// A same-language catalogue with a variant subtag is a valid fallback.
#[rstest]
fn variant_bearing_catalogue_supplies_a_same_language_summary(package_root: PackageRoot) {
    package_root.add_catalogue("en-GB-oxendict", "demo-about = Oxford spelling summary.\n");
    let args = args_for(&["en"]);

    let selected = select(&args, &package_root.path, None);

    assert_selection(selected, "en-GB-oxendict", "Oxford spelling summary.");
}

#[rstest]
fn lowercase_locale_directory_is_loaded_for_the_canonical_identifier(package_root: PackageRoot) {
    package_root.add_catalogue("en-gb", "demo-about = Lowercase British summary.\n");
    let args = args_for(&["en-GB"]);

    let selected = select(&args, &package_root.path, None);

    assert_selection(selected, "en-GB", "Lowercase British summary.");
}

#[rstest]
fn underscore_locale_directory_is_loaded_for_the_canonical_identifier(package_root: PackageRoot) {
    package_root.add_catalogue("en_GB", "demo-about = Underscore British summary.\n");
    let args = args_for(&["en-GB"]);

    let selected = select(&args, &package_root.path, None);

    assert_selection(selected, "en-GB", "Underscore British summary.");
}

/// An explicit locale suppresses the metadata preference, even when invalid.
#[rstest]
fn unparseable_explicit_locale_does_not_fall_back_to_metadata(package_root: PackageRoot) {
    package_root.add_catalogue("fr-FR", "demo-about = Resume francais.\n");
    package_root.add_catalogue("en-US", "demo-about = American English summary.\n");
    let args = args_for(&["not a locale!"]);

    let selected = select(&args, &package_root.path, Some(vec!["fr-FR".to_owned()]));

    assert_en_us_selection(selected);
}

#[rstest]
fn script_bearing_catalogue_supplies_a_same_language_summary(package_root: PackageRoot) {
    package_root.add_catalogue("zh-Hans", "demo-about = Chinese summary.\n");
    let args = args_for(&["zh"]);

    let selected = select(&args, &package_root.path, None);

    assert_selection(selected, "zh-Hans", "Chinese summary.");
}

#[rstest]
fn requested_locale_wins_over_metadata_locale(package_root: PackageRoot) {
    package_root.add_catalogue("fr-FR", "demo-about = Resume francais.\n");
    package_root.add_catalogue("en-GB", "demo-about = British English summary.\n");
    let args = args_for(&["fr-FR"]);

    let selected = select(&args, &package_root.path, Some(vec!["en-GB".to_owned()]));

    assert_selection(selected, "fr-FR", "Resume francais.");
}

#[rstest]
fn regional_request_falls_back_to_its_language_variant(package_root: PackageRoot) {
    package_root.add_catalogue("fr-FR", "demo-about = Resume francais.\n");
    let args = args_for(&["fr-CA"]);

    let selected = select(&args, &package_root.path, None);

    assert_selection(selected, "fr-FR", "Resume francais.");
}

#[rstest]
fn malformed_preferred_catalogue_falls_back_to_a_valid_one(package_root: PackageRoot) {
    package_root.add_catalogue("en-GB", "demo-about = { $unclosed\n");
    package_root.add_catalogue("en-US", "demo-about = American English summary.\n");
    let args = args_for(&["en-GB"]);

    let selected = select(&args, &package_root.path, None);

    assert_selection(selected, "en-US", "American English summary.");
}

#[rstest]
fn package_without_catalogues_uses_embedded_defaults(package_root: PackageRoot) {
    let args = args_for(&[]);

    let selected = select(&args, &package_root.path, None).expect("embedded defaults should build");

    assert_eq!(
        selected.locale.to_string(),
        "en-US",
        "embedded defaults should report the historical en-US locale",
    );
    assert_eq!(
        selected.source,
        SummaryLocaleSource::EmbeddedDefaults,
        "the recorded source should identify the embedded fallback",
    );
    assert!(
        !selected.has_package_resources,
        "embedded defaults are not built from package resources",
    );
    assert_eq!(
        selected.localizer.lookup("cli-about", None).as_deref(),
        Some("OrthoConfig command-line interface."),
        "embedded defaults should still resolve library messages",
    );
}

#[rstest]
fn non_agent_context_formats_skip_selection(package_root: PackageRoot) {
    package_root.add_catalogue("en-GB", "demo-about = British English summary.\n");
    let mut args = args_for(&["en-GB"]);
    args.format = OutputFormat::Ir;

    let selected = select(&args, &package_root.path, None);

    assert!(
        selected.is_none(),
        "a format without agent-context output should not build a summary localizer",
    );
}
