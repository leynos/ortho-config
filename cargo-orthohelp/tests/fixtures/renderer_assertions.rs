//! Wire assertions for the declared renderer block.
//!
//! Declared here rather than inside `fixtures` because only the renderer
//! end-to-end targets read renderer metadata; the other targets that include
//! the shared harness would otherwise carry these helpers as dead code.

use serde_json::Value;

type TestError = Box<dyn std::error::Error + Send + Sync>;

/// The renderer block `nested-fixture greet` declares, as it appears on the wire.
///
/// Written as a literal rather than derived from the fixture so every caller
/// pins the wire spelling a consumer depends on.
pub(crate) fn expected_greet_renderer() -> Value {
    serde_json::json!({
        "human": {
            "supported": true,
            "tty": "adapts",
            "colour": "auto",
            "width": {"kind": "terminal"},
            "plain": "available",
            "stdout": "text",
            "stderr": "text",
            "fields": [
                {"id": "message", "localization": "localized"},
                {"id": "error_code", "localization": "non_localized"}
            ]
        },
        "machine": {
            "supported": true,
            "tty": "independent",
            "colour": "never",
            "progress": "never",
            "pager": "never",
            "width": {"kind": "unbounded"},
            "plain": "always",
            "stdout": "structured",
            "stderr": "structured",
            "fields": [
                {"id": "error_code", "localization": "non_localized"}
            ]
        }
    })
}

/// Requires `document` to carry exactly the declared `greet` renderer.
///
/// `origin` names the artefact in the error so a failure says which writer
/// dropped or altered the metadata.
pub(crate) fn assert_greet_renderer(document: &Value, origin: &str) -> Result<(), TestError> {
    let greet = command_by_leaf(document, "greet").map_err(|error| format!("{origin}: {error}"))?;
    let renderer = greet
        .get("renderer")
        .ok_or_else(|| format!("{origin}: greet command carries no renderer block"))?;
    let declared = expected_greet_renderer();
    if *renderer == declared {
        Ok(())
    } else {
        Err(format!("{origin}: greet renderer should be {declared}, got {renderer}").into())
    }
}

/// Finds the single command whose `path` ends in `leaf`.
///
/// Requiring uniqueness means a renamed or duplicated command fails here rather
/// than silently asserting whichever match came first.
pub(crate) fn command_by_leaf<'a>(document: &'a Value, leaf: &str) -> Result<&'a Value, TestError> {
    let matches = document
        .get("commands")
        .and_then(Value::as_array)
        .ok_or("commands should be an array")?
        .iter()
        .filter(|command| {
            command
                .get("path")
                .and_then(Value::as_array)
                .and_then(|path| path.last())
                .and_then(Value::as_str)
                == Some(leaf)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [command] => Ok(command),
        other => Err(format!("expected one command named {leaf:?}, found {}", other.len()).into()),
    }
}
