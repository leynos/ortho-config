//! Trybuild coverage for the compile-time localization public paths.

/// Locks the public localization paths for downstream implementors.
///
/// The pass fixture must compile against the crate-root re-exports; the
/// compile-fail fixture pins the duplicate-id diagnostic so a loosened
/// validation rule cannot pass silently.
#[test]
fn localization_public_paths_compile() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/localization_public_paths.rs");
    t.compile_fail("tests/ui/localization_id_collision.rs");
}
