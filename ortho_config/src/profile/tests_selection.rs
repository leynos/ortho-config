//! Tests for selection resolution: flag beats environment, empty means
//! unset (milestone 3).

use pretty_assertions::assert_eq;
use rstest::rstest;

use crate::profile::{ProfileSource, SelectedProfile};

#[test]
fn flag_beats_environment() {
    let selection = SelectedProfile::resolve(Some("ci"), Some("local"))
        .expect("valid selections resolve")
        .expect("a selection is produced");
    assert_eq!(selection.source, ProfileSource::Flag);
    assert_eq!(selection.name.to_string(), "ci");
}

#[test]
fn environment_used_when_flag_absent() {
    let selection = SelectedProfile::resolve(None, Some("local"))
        .expect("valid selections resolve")
        .expect("a selection is produced");
    assert_eq!(selection.source, ProfileSource::Environment);
    assert_eq!(selection.name.to_string(), "local");
}

#[rstest]
#[case::no_sources(None, None)]
#[case::empty_flag(Some(""), Some("local"))]
#[case::empty_env(None, Some(""))]
#[case::default_flag(Some("default"), Some("local"))]
#[case::default_env(None, Some("default"))]
fn empty_or_reserved_means_unset(#[case] flag: Option<&str>, #[case] env: Option<&str>) {
    let selection = SelectedProfile::resolve(flag, env).expect("unset resolves without error");
    assert_eq!(selection, None);
}

#[test]
fn invalid_flag_name_is_rejected() {
    let err = SelectedProfile::resolve(Some("bad name"), None)
        .expect_err("an invalid name must be rejected");
    assert!(matches!(*err, crate::OrthoError::InvalidProfileName { .. }));
}

#[test]
fn invalid_environment_name_is_rejected() {
    let err = SelectedProfile::resolve(None, Some("bad name"))
        .expect_err("an invalid name must be rejected");
    assert!(matches!(*err, crate::OrthoError::InvalidProfileName { .. }));
}
