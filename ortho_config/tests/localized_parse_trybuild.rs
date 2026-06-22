//! trybuild coverage for the localised clap parsing public API.

#[test]
fn localized_parse_compile_time_contracts() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/localized_parse_parser.rs");
    t.compile_fail("tests/trybuild/localized_parse_requires_parser.rs");
}
