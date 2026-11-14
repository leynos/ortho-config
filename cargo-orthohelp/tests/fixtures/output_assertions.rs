//! Wire assertions for the declared structured-output block.
//!
//! Declared here rather than inside `fixtures` because only the
//! structured-output end-to-end target reads this metadata; the other targets
//! that include the shared harness would otherwise carry these helpers as dead
//! code.
//!
//! Every expectation is a literal rather than a value derived from the fixture,
//! so the assertions pin the wire spelling a consumer depends on. Three
//! commands are covered because the interesting cases are the differences
//! between them: `greet` declares a capability, `audit` explicitly refuses one,
//! and `version` says nothing at all.

use serde_json::{Value, json};

use super::wire_assertions::command_by_leaf;

type TestError = Box<dyn std::error::Error + Send + Sync>;

/// The output block `nested-fixture greet` declares, as it appears on the wire.
pub(crate) fn expected_greet_output() -> Value {
    json!({
        "supports_json": true,
        "stdout": "data",
        "stderr_diagnostics": true,
        "exit_classes": [
            {"name": "success", "meaning": "The greeting was printed"},
            {"name": "usage", "meaning": "The invocation was invalid"}
        ]
    })
}

/// Requires `document` to carry exactly the declared `greet` output block.
pub(crate) fn assert_greet_output(document: &Value, origin: &str) -> Result<(), TestError> {
    let greet = command_by_leaf(document, "greet").map_err(|error| format!("{origin}: {error}"))?;
    let output = greet
        .get("output")
        .ok_or_else(|| format!("{origin}: greet command carries no output block"))?;
    let declared = expected_greet_output();
    if *output == declared {
        Ok(())
    } else {
        Err(format!("{origin}: greet output should be {declared}, got {output}").into())
    }
}

/// Requires `document` to keep `audit`'s explicit refusal as a declared block.
///
/// A refusal and a silence are both "no JSON here" to a careless reader, so the
/// assertion checks that the block exists and says `false`, rather than only
/// checking that nothing claims `true`.
pub(crate) fn assert_audit_refusal(document: &Value, origin: &str) -> Result<(), TestError> {
    let audit = command_by_leaf(document, "audit").map_err(|error| format!("{origin}: {error}"))?;
    let output = audit
        .get("output")
        .ok_or_else(|| format!("{origin}: audit declares a refusal, so a block should exist"))?;

    if output.get("supports_json").and_then(Value::as_bool) != Some(false) {
        return Err(
            format!("{origin}: audit should declare supports_json false, got {output}").into(),
        );
    }
    if output.get("stdout").and_then(Value::as_str) != Some("summary") {
        return Err(
            format!("{origin}: audit should declare a summary stdout, got {output}").into(),
        );
    }
    Ok(())
}

/// Requires `document` to leave `version` with no output block at all.
///
/// Absence is the point: a bridge that defaulted an undeclared command to
/// `supports_json: false` would make silence indistinguishable from refusal.
pub(crate) fn assert_version_silence(document: &Value, origin: &str) -> Result<(), TestError> {
    let version =
        command_by_leaf(document, "version").map_err(|error| format!("{origin}: {error}"))?;

    match version.get("output") {
        None => Ok(()),
        Some(invented) => Err(format!("{origin}: version declared nothing, got {invented}").into()),
    }
}
