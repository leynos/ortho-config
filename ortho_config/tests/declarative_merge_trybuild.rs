//! trybuild coverage for declarative merge generation.
//!
//! Ensures that structs deriving `OrthoConfig` automatically expose the
//! `merge_from_layers` helper with collection strategies honoured at compile
//! time.

#[test]
fn declarative_merge_compiles_with_collection_strategies() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/declarative_merge_success.rs");
    t.compile_fail("tests/trybuild/declarative_merge_fail.rs");
}
