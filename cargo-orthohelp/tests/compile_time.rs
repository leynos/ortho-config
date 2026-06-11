//! Compile-time UI tests for `cargo-orthohelp`.
//!
//! These tests use `trybuild` to assert that code patterns which should be
//! rejected by the compiler (e.g. ignoring `#[must_use]` return values) do in
//! fact fail to compile.

#[test]
fn must_use_compile_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/must_use_bridge_ir.rs");
}
