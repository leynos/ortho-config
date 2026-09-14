//! trybuild coverage for generated lint attributes.
//!
//! A crate that allows dead code must not receive an unfulfilled expectation
//! from `OrthoConfig`'s generated compose-layer helpers.

#[test]
fn generated_compose_helpers_do_not_expect_dead_code() {
    let test_cases = trybuild::TestCases::new();
    test_cases.pass("tests/trybuild/allow_dead_code.rs");
}
