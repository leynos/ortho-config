//! Tests for composing declarative globals in the hello world example.

#[test]
#[should_panic(expected = "unknown provenance unknown")]
fn compose_declarative_globals_panics_on_invalid_provenance() {
    let mut world = crate::World::default();
    super::compose_declarative_globals_from_contents(
        &mut world,
        r#"[
            {"provenance": "unknown", "value": {"foo": "bar"}}
        ]"#,
    );
}

#[test]
#[should_panic(expected = "valid JSON describing declarative layers")]
fn compose_declarative_globals_panics_on_malformed_json() {
    let mut world = crate::World::default();
    super::compose_declarative_globals_from_contents(&mut world, "not valid json");
}
