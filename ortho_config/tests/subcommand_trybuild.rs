//! trybuild coverage for the subcommand merge public contracts.
//!
//! The compile-time check is the point: it pins the bounds on the
//! `SubcmdConfigMerge` trait and on the injected `_at` loaders from *outside*
//! the crate, so a requirement that stops being satisfiable from a downstream
//! call site is caught here rather than by every consumer.

#[test]
fn subcommand_merge_contracts_compile() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/subcommand_merge_success.rs");
    t.compile_fail("tests/trybuild/subcommand_merge_requires_extractor.rs");
}
