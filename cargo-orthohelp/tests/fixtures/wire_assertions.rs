//! Wire lookups shared by the end-to-end metadata assertions.
//!
//! Both the renderer and the structured-output assertions locate a command the
//! same way, so the lookup lives here rather than being copied into each; a
//! copied lookup could drift and let one suite assert the wrong command.

use serde_json::Value;

type TestError = Box<dyn std::error::Error + Send + Sync>;

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
