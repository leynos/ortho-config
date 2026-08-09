//! Tests for profile name grammar and reserved names (milestone 3).

use pretty_assertions::assert_eq;
use proptest::prelude::*;
use rstest::rstest;

use crate::OrthoError;
use crate::profile::ProfileName;

/// Whether `err` is an `InvalidProfileName` error.
///
/// Kept out of the property assertion so `prop_assert!` does not confuse the
/// `{ .. }` pattern with a format placeholder.
fn is_invalid_name_error(err: &OrthoError) -> bool {
    matches!(err, OrthoError::InvalidProfileName { .. })
}

#[test]
fn accepts_valid_names() {
    for name in ["ci", "weekly-recap", "staging_2", "UPPER", "a-b_c9"] {
        let profile = ProfileName::new(name).expect("valid name should be accepted");
        assert_eq!(profile.as_str(), name);
    }
}

#[rstest]
#[case::empty("")]
#[case::space("ci branch")]
#[case::dot("weekly.recap")]
#[case::alnum_unicode("プロファイル")]
#[case::symbol("ci!")]
fn rejects_invalid_names(#[case] name: &str) {
    let err = ProfileName::new(name).expect_err("name should be rejected");
    assert!(
        matches!(&*err, OrthoError::InvalidProfileName { .. }),
        "name {name:?} should be rejected as invalid"
    );
}

#[test]
fn rejects_reserved_default_name() {
    let err = ProfileName::new("default").expect_err("name should be rejected");
    assert!(
        matches!(&*err, OrthoError::ReservedProfileName { .. }),
        "the reserved name default should be rejected"
    );
}

proptest! {
    /// Every string the grammar accepts (minus the reserved name) validates.
    #[test]
    fn accepts_grammar_generated_names(name in "[A-Za-z0-9_-]{1,32}") {
        prop_assume!(name != "default");
        prop_assert!(ProfileName::new(&name).is_ok());
    }

    /// Every string outside the grammar is rejected as invalid.
    #[test]
    fn rejects_non_grammar_names(name in r"[^A-Za-z0-9_-]{1,16}") {
        prop_assert!(
            ProfileName::new(&name)
                .err()
                .as_deref()
                .is_some_and(is_invalid_name_error)
        );
    }
}
