//! Unit tests for locale resolution and Fluent resource loading.

use super::*;
use crate::cli_test_support;
use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest::{fixture, rstest};
use tempfile::TempDir;

/// Injects the shared default CLI arguments as an rstest fixture.
#[fixture]
fn base_args() -> Args {
    cli_test_support::base_args()
}

/// Builds a selection whose metadata declares `locales`.
fn selection_with_locales(
    package_root: Utf8PathBuf,
    locales: Option<Vec<String>>,
) -> PackageSelection {
    cli_test_support::selection_with_locales(package_root, locales)
}

#[rstest]
fn defaults_to_en_us(base_args: Args) {
    let selection = selection_with_locales(Utf8PathBuf::from("."), None);

    let locales = resolve_locales(&base_args, &selection).expect("resolve locales");
    assert_eq!(
        locales.len(),
        1,
        "without flags or metadata the resolver should return one locale",
    );
    let locale = locales.first().expect("expected one locale");
    assert_eq!(
        locale.to_string(),
        "en-US",
        "the default locale should stay en-US",
    );
}

#[rstest]
fn uses_metadata_locales_when_requested(mut base_args: Args) {
    base_args.should_use_all_locales = true;
    let selection = selection_with_locales(Utf8PathBuf::from("."), Some(vec!["fr-FR".to_owned()]));

    let locales = resolve_locales(&base_args, &selection).expect("resolve locales");
    assert_eq!(
        locales.len(),
        1,
        "--all-locales with one metadata locale should return one locale",
    );
    let locale = locales.first().expect("expected one locale");
    assert_eq!(
        locale.to_string(),
        "fr-FR",
        "metadata should supply the requested locale",
    );
}

#[rstest]
#[case(
    vec!["ja-JP".to_owned()],
    vec!["ja-JP".to_owned()]
)]
#[case(
    vec!["en-US".to_owned(), "en-US".to_owned(), "fr-FR".to_owned()],
    vec!["en-US".to_owned(), "fr-FR".to_owned()]
)]
fn uses_cli_locales_and_dedupes(
    mut base_args: Args,
    #[case] requested: Vec<String>,
    #[case] expected: Vec<String>,
) {
    base_args.locale = requested;
    let selection = selection_with_locales(Utf8PathBuf::from("."), None);

    let locales = resolve_locales(&base_args, &selection).expect("resolve locales");
    let resolved = locales
        .into_iter()
        .map(|locale| locale.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        resolved, expected,
        "explicit locales should be deduplicated in request order",
    );
}

#[rstest]
fn discovers_locales_when_metadata_missing(mut base_args: Args) {
    let temp_dir = TempDir::new().expect("temp dir");
    let package_root =
        Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).expect("temp dir is UTF-8");
    let dir =
        Dir::open_ambient_dir(package_root.as_path(), ambient_authority()).expect("open temp dir");
    dir.create_dir_all("locales/en-US")
        .expect("create locale dir");
    dir.create_dir_all("locales/fr-FR")
        .expect("create locale dir");

    base_args.should_use_all_locales = true;
    let selection = selection_with_locales(package_root, None);

    let locales = resolve_locales(&base_args, &selection).expect("resolve locales");
    let resolved = locales
        .into_iter()
        .map(|locale| locale.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        resolved,
        vec!["en-US".to_owned(), "fr-FR".to_owned()],
        "discovered locale directories should be sorted and deduplicated",
    );
}
