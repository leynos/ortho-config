//! trybuild coverage for `#[ortho_config(crate = "...")]` support.
//!
//! Ensures that the `crate` attribute is accepted by the derive macro and
//! that generated code compiles correctly when the crate path is overridden.

#[test]
fn crate_path_alias_compiles() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/crate_path_alias.rs");
    t.pass("tests/trybuild/crate_path_alias_renamed.rs");
}
