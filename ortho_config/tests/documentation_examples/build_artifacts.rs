//! Stream Cargo artefacts without retaining unbounded compiler output.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MAX_MESSAGE_BYTES: usize = 128 * 1024;

#[derive(Default)]
pub(super) struct Artifacts {
    pending: Vec<u8>,
    oversized: bool,
    invalid: bool,
    executables: BTreeMap<String, PathBuf>,
}

impl Artifacts {
    /// Consume stdout chunks; an oversized JSON record makes extraction fail.
    pub(super) fn consume(&mut self, bytes: &[u8]) {
        for byte in bytes {
            if *byte == b'\n' {
                self.finish_line();
            } else if self.pending.len() < MAX_MESSAGE_BYTES {
                self.pending.push(*byte);
            } else {
                self.oversized = true;
            }
        }
    }

    fn finish_line(&mut self) {
        if !self.pending.is_empty() && !self.oversized {
            match serde_json::from_slice::<serde_json::Value>(&self.pending) {
                Ok(value) => self.record(&value),
                Err(_) => self.invalid = true,
            }
        }
        self.pending.clear();
        if self.oversized {
            self.invalid = true;
            self.oversized = false;
        }
    }

    fn record(&mut self, value: &serde_json::Value) {
        if value.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-artifact") {
            return;
        }
        let Some(executable) = value.get("executable").and_then(serde_json::Value::as_str) else {
            return;
        };
        let Some(target) = value.get("target") else {
            return;
        };
        let is_binary = target
            .get("kind")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin")));
        if let Some(name) = target.get("name").and_then(serde_json::Value::as_str)
            && self.can_record_binary(is_binary)
        {
            self.executables
                .insert(name.to_owned(), PathBuf::from(executable));
        } else if is_binary {
            self.invalid = true;
        }
    }

    fn can_record_binary(&self, is_binary: bool) -> bool {
        is_binary && self.executables.len() < 128
    }

    /// Snapshot every JSON-reported executable while the cache lock is held.
    pub(super) fn snapshot(&mut self, destination: &Path) -> Result<BTreeMap<String, PathBuf>> {
        self.finish_line();
        ensure!(
            !self.invalid,
            "Cargo JSON contains an invalid or oversized record"
        );
        ensure!(
            !self.executables.is_empty(),
            "Cargo JSON reported no executables"
        );
        Dir::create_ambient_dir_all(destination, ambient_authority())?;
        let directory = Dir::open_ambient_dir(destination, ambient_authority())?;
        let mut snapshots = BTreeMap::new();
        for (name, executable) in &self.executables {
            ensure!(
                executable.is_absolute(),
                "Cargo executable path must be absolute"
            );
            let file_name = executable
                .file_name()
                .context("Cargo executable has no filename")?;
            let source = Dir::open_ambient_dir(
                executable
                    .parent()
                    .context("Cargo executable has no directory")?,
                ambient_authority(),
            )?;
            source
                .copy(file_name, &directory, file_name)
                .with_context(|| format!("snapshot Cargo executable {name}"))?;
            snapshots.insert(name.clone(), destination.join(file_name));
        }
        Ok(snapshots)
    }
}

#[cfg(test)]
#[path = "build_artifacts_tests.rs"]
mod tests;
