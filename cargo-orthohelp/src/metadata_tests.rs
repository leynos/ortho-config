//! Unit tests for the metadata root type parser.

use super::*;
use rstest::rstest;

#[rstest]
#[case::crate_prefix("crate::Config", "demo", "demo::Config")]
#[case::bare_type("Config", "demo", "demo::Config")]
#[case::qualified("demo::Config", "ignored", "demo::Config")]
#[case::nested_crate_prefix("crate::config::Config", "demo", "demo::config::Config")]
#[case::whitespace_is_canonicalized("demo :: Config", "ignored", "demo::Config")]
fn parses_root_type(#[case] raw: &str, #[case] crate_ident: &str, #[case] expected: &str) {
    let parsed = parse_root_type(raw, crate_ident).expect("root type should parse");
    assert_eq!(parsed, expected);
}

#[rstest]
#[case::generic("demo::Config<T>", "generic or parenthesized arguments")]
#[case::tuple("(A, B)", "failed to parse root type")]
#[case::reference("&Config", "failed to parse root type")]
#[case::empty("", "failed to parse root type")]
#[case::qualified_self(
    "<Config as Trait>::Assoc",
    "qualified self types such as `<Type as Trait>::Assoc` are not supported"
)]
#[case::global_path("::std::Config", "global paths starting with `::` are not supported")]
#[case::injection(
    "Config as OrthoConfigDocs>::get_doc_metadata(); std::process::exit(1); let _ = <Config",
    "failed to parse root type"
)]
fn rejects_unsupported_root_type(#[case] raw: &str, #[case] expected_message: &str) {
    let error = parse_root_type(raw, "demo").expect_err("root type should be rejected");
    let message = error.to_string();
    assert!(
        message.contains(expected_message),
        "unexpected diagnostic: {message}"
    );
}
