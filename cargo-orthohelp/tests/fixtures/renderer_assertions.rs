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
            "closed_stdin": "eof",
            "colour": "auto",
            "emoji": "never",
            "progress": "always",
            "pager": "auto",
            "width": {"kind": "terminal"},
            "accessibility": "text_equivalent",
            "plain": "available",
            "stdout": "text",
            "stderr": "mixed",
            "progress_inheritance": "allowed",
            "subprocess_inheritance": "forbidden",
            "fields": [
                {"id": "message", "localization": "localized"},
                {"id": "error_code", "localization": "non_localized"}
            ]
        },
        "machine": {
            "supported": true,
            "tty": "independent",
            "closed_stdin": "error",
            "colour": "never",
            "emoji": "auto",
            "progress": "never",
            "pager": "always",
            "width": {"kind": "unbounded"},
            "accessibility": "unspecified",
            "plain": "always",
            "stdout": "structured",
            "stderr": "none",
            "progress_inheritance": "forbidden",
            "subprocess_inheritance": "allowed",
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

pub(crate) use super::wire_assertions::command_by_leaf;
