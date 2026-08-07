//! trybuild coverage for the public `EnvSource` contract.
//!
//! The compile-time check is the point: it pins object safety, the
//! `Debug + Send + Sync` supertraits, the owned `Option<OsString>` return, and
//! the builder's acceptance of `SharedEnvSource` from *outside* the crate,
//! using a bespoke implementor rather than `MapEnv`.

#[test]
fn bespoke_env_source_compiles() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/env_source_object_safe.rs");
}
