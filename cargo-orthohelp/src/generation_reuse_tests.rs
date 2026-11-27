//! Unit tests for provenance-gated reuse of the summary localizer.
//!
//! Human-document localization may reuse the summary localizer only when it
//! came from package resources and the requested locale matches exactly. These
//! tests drive `localize_docs_if_requested` with a minimal document, so the
//! reuse decision is observed through the returned localized text rather than
//! through a predicate alone.

use super::*;
use crate::cli_test_support;
use crate::cli_test_support::PackageRoot;
use crate::summary_locale::SummaryLocalizer;
use rstest::{fixture, rstest};

const ABOUT_ID: &str = "demo-about";

/// Builds an empty package root for one test.
#[fixture]
fn package_root() -> PackageRoot {
    PackageRoot::new()
}

/// Builds arguments that request the agent-context format.
fn args_for(locales: &[&str]) -> Args {
    let mut args = cli_test_support::base_args();
    args.format = OutputFormat::AgentContext;
    args.locale = locales.iter().map(|locale| (*locale).to_owned()).collect();
    args
}

/// Selects the summary localizer through the generation boundary.
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

/// Builds the smallest document the localization path accepts.
fn minimal_doc_metadata() -> DocMetadata {
    serde_json::from_value(serde_json::json!({
        "ir_version": "2.0",
        "app_name": "demo",
        "bin_name": null,
        "about_id": ABOUT_ID,
        "synopsis_id": null,
        "sections": {
            "headings_ids": {
                "name": "ortho.headings.name",
                "synopsis": "ortho.headings.synopsis",
                "description": "ortho.headings.description",
                "options": "ortho.headings.options",
                "environment": "ortho.headings.environment",
                "files": "ortho.headings.files",
                "precedence": "ortho.headings.precedence",
                "exit_status": "ortho.headings.exit_status",
                "examples": "ortho.headings.examples",
                "see_also": "ortho.headings.see_also"
            },
            "examples": [],
            "links": [],
            "notes": []
        },
        "fields": [],
        "subcommands": [],
        "windows": null
    }))
    .expect("the minimal document should deserialize")
}

/// Runs human-document localization with `selected` as the cached localizer.
fn localize_with(
    package_root: &Utf8PathBuf,
    selected: &SummaryLocalizer,
    locale: &LanguageIdentifier,
) -> Result<Vec<LocalizedDocMetadata>, OrthohelpError> {
    localize_many_with(package_root, selected, std::slice::from_ref(locale))
}

/// Runs human-document localization for several locales in one request.
fn localize_many_with(
    package_root: &Utf8PathBuf,
    selected: &SummaryLocalizer,
    locales: &[LanguageIdentifier],
) -> Result<Vec<LocalizedDocMetadata>, OrthohelpError> {
    let selection = cli_test_support::selection_with_locales(package_root.clone(), None);
    let doc_metadata = minimal_doc_metadata();
    let context = GenerationContext {
        selection: &selection,
        doc_metadata: &doc_metadata,
        out_dir: package_root,
        summary_localizer: Some(selected),
    };
    localize_docs_if_requested(true, &context, locales)
}

#[rstest]
fn package_catalogue_is_reusable_for_human_documents(package_root: PackageRoot) {
    package_root.add_catalogue("en-US", "demo-about = American English summary.\n");
    let args = args_for(&["en-US"]);
    let selected =
        select(&args, &package_root.path, None).expect("the package catalogue should build");
    let en_us = ortho_config::langid!("en-US");

    assert!(
        selected.is_reusable_for(&en_us),
        "a catalogue built from package resources should be eligible for reuse",
    );
    let localized = localize_with(&package_root.path, &selected, &en_us)
        .expect("a valid catalogue should localize");
    assert_eq!(
        localized.first().map(|doc| doc.about.as_str()),
        Some("American English summary."),
        "the reused localizer should supply the localized about text",
    );
}

#[rstest]
fn malformed_catalogue_is_not_reused_for_human_documents(package_root: PackageRoot) {
    package_root.add_catalogue("en-US", "demo-about = { $unclosed\n");
    let args = args_for(&["en-US"]);
    let selected = select(&args, &package_root.path, None)
        .expect("embedded defaults should still supply summaries");
    let en_us = ortho_config::langid!("en-US");

    assert!(
        !selected.is_reusable_for(&en_us),
        "the embedded fallback must not stand in for the malformed package catalogue",
    );
    let result = localize_with(&package_root.path, &selected, &en_us);
    assert!(
        result.is_err(),
        "human localization should still report the malformed catalogue, got {result:?}",
    );
}

/// A summary localizer for one locale must not localize another locale.
#[rstest]
fn non_matching_locale_is_not_reused(package_root: PackageRoot) {
    package_root.add_catalogue("en-GB", "demo-about = British English summary.\n");
    package_root.add_catalogue("en-US", "demo-about = American English summary.\n");
    let args = args_for(&["en-GB"]);
    let selected =
        select(&args, &package_root.path, None).expect("the en-GB catalogue should build");
    let en_gb = ortho_config::langid!("en-GB");
    let en_us = ortho_config::langid!("en-US");

    assert!(
        selected.is_reusable_for(&en_gb),
        "the en-GB catalogue should be reusable for en-GB",
    );
    assert!(
        !selected.is_reusable_for(&en_us),
        "the en-GB catalogue must not stand in for en-US",
    );
    let localized = localize_with(&package_root.path, &selected, &en_us)
        .expect("the en-US catalogue should localize");
    assert_eq!(
        localized.first().map(|doc| doc.about.as_str()),
        Some("American English summary."),
        "en-US output must come from the en-US catalogue, not the cached en-GB text",
    );
}

/// A multi-locale request reuses the cached localizer only for its own locale.
#[rstest]
fn multi_locale_localization_reuses_only_the_exact_locale(package_root: PackageRoot) {
    package_root.add_catalogue("en-GB", "demo-about = British English summary.\n");
    package_root.add_catalogue("en-US", "demo-about = American English summary.\n");
    let args = args_for(&["en-GB"]);
    let selected =
        select(&args, &package_root.path, None).expect("the en-GB catalogue should build");
    let locales = [
        ortho_config::langid!("en-GB"),
        ortho_config::langid!("en-US"),
    ];

    let localized = localize_many_with(&package_root.path, &selected, &locales)
        .expect("both catalogues should localize");

    let abouts = localized
        .iter()
        .map(|doc| (doc.locale.as_str(), doc.about.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        abouts,
        vec![
            ("en-GB", "British English summary."),
            ("en-US", "American English summary."),
        ],
        "each locale should use its own catalogue",
    );
}
