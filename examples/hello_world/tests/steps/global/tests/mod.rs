//! Tests for composing declarative globals in the hello world example.
use anyhow::Result;
use rstest::{fixture, rstest};

#[fixture]
fn world() -> Result<crate::World> {
    crate::World::for_tests()
}

#[rstest]
fn compose_declarative_globals_rejects_invalid_provenance(
    world: Result<crate::World>,
) -> Result<()> {
    let mut world = world?;
    let result = super::compose_declarative_globals_from_contents(
        &mut world,
        r#"[
            {"provenance": "unknown", "value": {"foo": "bar"}}
        ]"#,
    );
    let Err(err) = result else {
        return Err(anyhow::anyhow!(
            "expected provenance error when composing declarative globals"
        ));
    };
    anyhow::ensure!(err.to_string().contains("unknown provenance"));
    Ok(())
}

#[rstest]
fn compose_declarative_globals_rejects_malformed_json(world: Result<crate::World>) -> Result<()> {
    let mut world = world?;
    let result = super::compose_declarative_globals_from_contents(&mut world, "not valid json");
    let Err(err) = result else {
        return Err(anyhow::anyhow!(
            "expected JSON parsing error when composing declarative globals"
        ));
    };
    anyhow::ensure!(err.to_string().contains("valid JSON"));
    Ok(())
}
