//! trybuild coverage for the generated optional-value boolean CLI.
//!
//! Unit tests exercise the derived parser from inside the crate; this pass
//! fixture compiles the same expansion from a downstream-style crate, which is
//! the only way to catch an emitted path or bound that resolves privately but
//! not publicly. See `tests/trybuild/boolean_cli_override.rs`.

#[test]
fn boolean_cli_contracts_compile() {
    trybuild::TestCases::new().pass("tests/trybuild/boolean_cli_override.rs");
}
