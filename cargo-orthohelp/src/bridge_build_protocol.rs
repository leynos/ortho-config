//! Read Cargo's bounded JSON stream and identify the bridge executable.
//!
//! This protocol reader drains every record so Cargo can finish, while keeping
//! malformed and oversized messages as a bounded build failure.

use camino::Utf8PathBuf;
use serde_json::Value;
use std::io::{self, BufRead, Write};

use super::{GeneratedTarget, binary_file_name};
use crate::error::OrthohelpError;

/// Maximum retained size for one newline-delimited Cargo JSON record.
const MAX_CARGO_RECORD_BYTES: usize = 128 * 1024;
/// Relevant build output collected while draining Cargo's JSON stream.
pub(super) struct CargoMessages {
    /// Absolute path reported for the bridge executable, when present.
    pub(super) executable: Option<Utf8PathBuf>,
    /// First malformed, oversized, or out-of-scope protocol record observed.
    pub(super) protocol_error: Option<OrthohelpError>,
}

/// Bounded bytes and size state for one Cargo JSON line.
struct CargoRecord {
    /// Retained prefix of the record, capped at `MAX_CARGO_RECORD_BYTES`.
    bytes: Vec<u8>,
    /// Whether bytes beyond the retained prefix were discarded.
    oversized: bool,
}

/// Drain Cargo JSON into an explicit diagnostic sink and locate the bridge binary.
pub(super) fn read_cargo_messages<R: BufRead>(
    reader: &mut R,
    target: &GeneratedTarget<'_>,
    diagnostic_sink: &mut impl Write,
) -> io::Result<CargoMessages> {
    let mut messages = CargoMessages {
        executable: None,
        protocol_error: None,
    };
    while let Some(record) = read_record(reader)? {
        if record.oversized {
            set_protocol_error(&mut messages, "Cargo JSON record exceeded 128 KiB");
            continue;
        }
        parse_record(&record.bytes, target, &mut messages, diagnostic_sink)?;
    }
    diagnostic_sink.flush()?;

    Ok(messages)
}

/// Read one complete line while retaining no more than the protocol limit.
fn read_record<R: BufRead>(reader: &mut R) -> io::Result<Option<CargoRecord>> {
    let mut bytes = Vec::new();
    let mut oversized = false;

    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if bytes.is_empty() && !oversized {
                Ok(None)
            } else {
                Ok(Some(CargoRecord { bytes, oversized }))
            };
        }

        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |index| index + 1);
        if !oversized {
            let remaining = MAX_CARGO_RECORD_BYTES.saturating_sub(bytes.len());
            let copied = consumed.min(remaining);
            let prefix = available.get(..copied).unwrap_or_default();
            bytes.extend_from_slice(prefix);
            oversized = copied < consumed;
        }
        reader.consume(consumed);

        if newline.is_some() {
            return Ok(Some(CargoRecord { bytes, oversized }));
        }
    }
}

/// Parse a bounded Cargo message and handle only diagnostics and artefacts.
fn parse_record(
    bytes: &[u8],
    target: &GeneratedTarget<'_>,
    messages: &mut CargoMessages,
    stderr: &mut impl Write,
) -> io::Result<()> {
    let value = match serde_json::from_slice::<Value>(bytes) {
        Ok(value) => value,
        Err(error) => {
            set_protocol_error(messages, &format!("invalid Cargo JSON record: {error}"));
            return Ok(());
        }
    };

    match value.get("reason").and_then(Value::as_str) {
        Some("compiler-message") => forward_rendered_diagnostic(&value, stderr)?,
        Some("compiler-artifact") => {
            collect_executable(&value, target, messages);
        }
        _ => {}
    }
    Ok(())
}

/// Forward Cargo's human-readable compiler diagnostic to the supervising stderr.
fn forward_rendered_diagnostic(value: &Value, stderr: &mut impl Write) -> io::Result<()> {
    if let Some(rendered) = value
        .get("message")
        .and_then(|message| message.get("rendered"))
        .and_then(Value::as_str)
    {
        stderr.write_all(rendered.as_bytes())?;
    }
    Ok(())
}

/// Accept one compiler-artifact path when it identifies the expected binary.
fn collect_executable(value: &Value, target: &GeneratedTarget<'_>, messages: &mut CargoMessages) {
    if !is_bridge_binary_artifact(value, target.binary_name) {
        return;
    }
    let Some(executable) = artifact_executable(value, messages) else {
        return;
    };
    record_bridge_executable(executable, target, messages);
}

/// Identify artifact records that belong to the generated bridge binary.
fn is_bridge_binary_artifact(value: &Value, binary_name: &str) -> bool {
    let target = value.get("target");
    target
        .and_then(|target_value| target_value.get("name"))
        .and_then(Value::as_str)
        == Some(binary_name)
        && target
            .and_then(|target_value| target_value.get("kind"))
            .and_then(Value::as_array)
            .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin")))
}

/// Read the executable path, retaining a protocol error when it is absent.
fn artifact_executable(value: &Value, messages: &mut CargoMessages) -> Option<Utf8PathBuf> {
    value
        .get("executable")
        .and_then(Value::as_str)
        .map(Utf8PathBuf::from)
        .or_else(|| {
            set_protocol_error(
                messages,
                "bridge compiler-artifact record omitted executable path",
            );
            None
        })
}

/// Record one validated executable path and reject ambiguous build output.
fn record_bridge_executable(
    executable: Utf8PathBuf,
    target: &GeneratedTarget<'_>,
    messages: &mut CargoMessages,
) {
    if !executable.is_absolute() {
        set_protocol_error(messages, "bridge compiler-artifact path was not absolute");
        return;
    }
    match messages.executable.as_ref() {
        Some(previous) if previous == &executable => return,
        Some(_) => {
            set_protocol_error(
                messages,
                "Cargo reported multiple generated bridge executables",
            );
            return;
        }
        None => {}
    }
    if executable.file_name() != Some(binary_file_name(target.binary_name).as_str()) {
        set_protocol_error(
            messages,
            "Cargo bridge artifact had an unexpected generated executable name",
        );
        return;
    }
    if executable.starts_with(target.directory) {
        messages.executable = Some(executable);
    } else {
        set_protocol_error(
            messages,
            "Cargo bridge artifact was outside the configured target directory",
        );
    }
}

/// Preserve only the first protocol error so later records cannot obscure it.
fn set_protocol_error(messages: &mut CargoMessages, message: &str) {
    if messages.protocol_error.is_none() {
        messages.protocol_error = Some(OrthohelpError::Message(message.to_owned()));
    }
}

#[cfg(test)]
#[path = "bridge_build_protocol_tests.rs"]
mod tests;
