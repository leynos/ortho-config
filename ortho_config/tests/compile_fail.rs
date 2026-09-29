//! Compile-time checks for public API contracts and macro diagnostics.

/// Check that public APIs compile from downstream-style crates.
#[test]
fn public_api_contracts_compile_for_downstream_crates() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/public_api_contracts.rs");
    t.pass("tests/trybuild/source_aware_composition.rs");
}

#[test]
fn macro_errors_match_expected_diagnostics() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
