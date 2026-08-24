//! trybuild coverage for the public environment-source contracts.
//!
//! The compile-time check is the point: it pins object safety, the
//! `Debug + Send + Sync` supertraits, and the aliases' acceptance of bespoke
//! source implementations from *outside* the crate, rather than `MapEnv`.

#[test]
fn bespoke_environment_sources_compile() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/env_source_object_safe.rs");
    t.pass("tests/trybuild/scan_env_source_object_safe.rs");
}
