//! trybuild coverage for the public environment-source contracts.
//!
//! The compile-time check is the point: it pins object safety, the
//! `Debug + Send + Sync` supertraits, and the aliases' acceptance of bespoke
//! source implementations from *outside* the crate, rather than `MapEnv`.

/// Verifies public environment and selected-subcommand source contracts compile.
#[test]
fn environment_source_contracts_compile() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/env_source_object_safe.rs");
    t.pass("tests/trybuild/scan_env_source_object_safe.rs");
    t.pass("tests/trybuild/selected_subcommand_sources.rs");
}
