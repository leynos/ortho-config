//! Tests for Fluent resource parsing and identifier normalization helpers.

use super::*;
use rstest::rstest;
use unic_langid::langid;

// =============================================================================
// default_resources tests
// =============================================================================

#[rstest]
#[case::en_us(langid!("en-US"), EN_US_RESOURCES.as_slice(), "EN_US_RESOURCES", "en-US should return English resources")]
#[case::en_gb(langid!("en-GB"), EN_US_RESOURCES.as_slice(), "EN_US_RESOURCES", "en-GB should return English resources (language-based matching)")]
#[case::bare_en(langid!("en"), EN_US_RESOURCES.as_slice(), "EN_US_RESOURCES", "bare 'en' should return English resources")]
#[case::ja(langid!("ja"), JA_RESOURCES.as_slice(), "JA_RESOURCES", "ja should return Japanese resources")]
#[case::ja_jp(langid!("ja-JP"), JA_RESOURCES.as_slice(), "JA_RESOURCES", "ja-JP should return Japanese resources (language-based matching)")]
fn default_resources_returns_expected_catalogue(
    #[case] locale: LanguageIdentifier,
    #[case] expected_resources: &'static [&'static str],
    #[case] resource_name: &str,
    #[case] description: &str,
) {
    let resources = default_resources(&locale);
    assert!(
        resources.is_some(),
        "{description}: should return Some for locale {locale}"
    );
    assert!(
        std::ptr::eq(resources.expect("checked above"), expected_resources),
        "{description}: should return {resource_name} for locale {locale}"
    );
}

#[test]
fn default_resources_returns_none_for_unsupported_locale() {
    let locale = langid!("fr-FR");
    let resources = default_resources(&locale);
    assert!(resources.is_none(), "unsupported locale should return None");
}

// =============================================================================
// Identifier normalization tests
// =============================================================================

#[test]
fn normalizes_top_level_dotted_ids() {
    let resource = "cli.about = About text";
    let normalized = normalize_resource_ids(resource);
    assert_eq!(
        normalized, "cli-about = About text",
        "a top-level dotted id should be rewritten to its hyphenated spelling"
    );
}

#[test]
fn leaves_indented_lines_with_equals_untouched() {
    let resource = "message =\n    section.title = Title line";
    let normalized = normalize_resource_ids(resource);
    assert!(
        normalized.contains("    section.title = Title line"),
        "a continuation line should not be treated as a top-level id, got: {normalized}"
    );
}

#[test]
fn leaves_terms_and_attributes_unchanged() {
    let resource = "-term.id = Term value\nmessage = Value\n    .label = Label";
    let normalized = normalize_resource_ids(resource);
    assert!(
        normalized.contains("-term.id = Term value"),
        "a term id should keep its original spelling, got: {normalized}"
    );
    assert!(
        normalized.contains("    .label = Label"),
        "an attribute line should keep its original spelling, got: {normalized}"
    );
}

#[test]
fn normalizes_unicode_identifiers() {
    let resource = "ключ.значение = Привет";
    let normalized = normalize_resource_ids(resource);
    assert!(
        normalized.starts_with("ключ-значение = Привет"),
        "a non-ASCII top-level id should also be rewritten to its hyphenated spelling, got: {normalized}"
    );
}
