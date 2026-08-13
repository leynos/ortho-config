//! Trybuild coverage for the compile-time localization public paths.

#[test]
fn localization_public_paths_compile() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/localization_public_paths.rs");
}
