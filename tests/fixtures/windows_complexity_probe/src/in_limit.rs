//! The Windows-only example the gate must accept.

use std::path::Path;

/// Normalizes a path the way Windows compares them.
///
/// This mirrors `windows_normalized_key` in `ortho_config`'s discovery module,
/// whose shape the gate accepts today: one `match` with two explicit arms and
/// a wildcard, well inside the limit of 9. If this example ever fails, either
/// the threshold moved or the metric stopped recognising a `match` arm as a
/// single decision, and both are worth knowing before the workspace gate
/// reports the same thing.
pub fn in_limit(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .map(|unit| match unit {
            65..=90 => unit + 32,
            47 => 92,
            _ => unit,
        })
        .collect()
}