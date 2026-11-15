//! Compile-time contracts for the public process supervision helpers.

/// Keep the public process helpers' `must_use` contracts observable to callers.
#[test]
fn process_api_compile_contracts() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/process_api/pass.rs");
    cases.compile_fail("tests/ui/process_api/spawn_guard_new_must_use.rs");
    cases.compile_fail("tests/ui/process_api/spawn_guard_queries_must_use.rs");
    cases.compile_fail("tests/ui/process_api/reader_queries_must_use.rs");
}
