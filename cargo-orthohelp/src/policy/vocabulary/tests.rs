//! Tests for the canonical vocabulary defaults.

use super::*;
use rstest::rstest;

#[rstest]
#[case("get")]
#[case("list")]
#[case("create")]
#[case("update")]
#[case("delete")]
#[case("jobs")]
#[case("profile")]
#[case("feedback")]
fn canonical_verb_is_a_member(#[case] verb: &str) {
    assert!(is_canonical_verb(verb));
}

#[rstest]
#[case("info")]
#[case("ls")]
#[case("apply")]
fn non_canonical_verb_is_not_a_member(#[case] verb: &str) {
    assert!(!is_canonical_verb(verb));
}

#[rstest]
#[case("--json")]
#[case("--no-input")]
#[case("--force")]
#[case("--dry-run")]
#[case("--limit")]
#[case("--cursor")]
#[case("--wait")]
#[case("--profile")]
#[case("--deliver")]
fn canonical_flag_is_a_member(#[case] flag: &str) {
    assert!(is_canonical_flag(flag), "expected {flag} to be canonical");
}

#[rstest]
#[case("--format")]
#[case("--output")]
#[case("--skip-confirmations")]
fn non_canonical_flag_is_not_a_member(#[case] flag: &str) {
    assert!(
        !is_canonical_flag(flag),
        "expected {flag} not to be canonical"
    );
}

#[rstest]
#[case("json", true)]
#[case("--json", true)]
#[case("format", false)]
#[case("--format", false)]
fn flag_membership_accepts_optional_prefix(#[case] flag: &str, #[case] expected: bool) {
    assert_eq!(is_canonical_flag(flag), expected);
}
