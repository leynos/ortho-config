//! Closed build-tool environment policy for documentation tests.
//!
//! Cargo retains the user's configured shared cache and Cargo configuration.
//! Only named toolchain, compiler-admission, and platform variables cross this
//! boundary; application processes use the narrower policy in `workspace`.

use std::ffi::OsString;
use std::process::Command;

pub(super) const CARGO_ENV_ALLOWLIST: &[&str] = &[
    "BUILD_LIMITS_COMPILERS_SLICE",
    "BUILD_LIMITS_HEARTBEAT_SECS",
    "BUILD_LIMITS_RUSTUP",
    "BUILD_LIMITS_SOCKET",
    "BUILD_LIMITS_UNAVAILABLE_SECS",
    "CARGO_BUILD_JOBS",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTDOC",
    "CARGO_BUILD_RUSTDOCFLAGS",
    "CARGO_BUILD_RUSTFLAGS",
    "CARGO_ENCODED_RUSTDOCFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_HOME",
    "CARGO_INCREMENTAL",
    "CARGO_LLVM_COV",
    "CARGO_MAKEFLAGS",
    "HOME",
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "LLVM_PROFILE_FILE",
    "LLVM_PROFILE_MERGE_POOL_SIZE",
    "MAKEFLAGS",
    "PATH",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "RUSTC",
    "RUSTC_HEARTBEAT_SECS",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTDOC",
    "RUSTDOCFLAGS",
    "RUSTFLAGS",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "SCCACHE_GHA_ENABLED",
    "SYSTEMROOT",
    "TMPDIR",
    "USERPROFILE",
    "VCINSTALLDIR",
    "VSCMD_ARG_TGT_ARCH",
    "VSINSTALLDIR",
    "WINDIR",
    "WindowsSDKVersion",
    "WindowsSdkDir",
    "XDG_RUNTIME_DIR",
];

const METADATA_TARGET_ENV_ALLOWLIST: &[&str] = &[
    "CARGO_BUILD_TARGET_DIR",
    "CARGO_TARGET_DIR",
    "CARGO_LLVM_COV_TARGET_DIR",
];

/// A selected environment for a Cargo or host build-tool command.
pub(super) struct CargoEnvironment(Vec<(OsString, OsString)>);

impl CargoEnvironment {
    /// Capture only the closed toolchain, wrapper, and platform allow-list.
    pub(super) fn build() -> Self {
        Self::from_lookup(CARGO_ENV_ALLOWLIST, |name| std::env::var_os(name))
    }

    /// Capture the build environment plus caller target paths for metadata.
    pub(super) fn metadata() -> Self {
        Self::metadata_with_lookup(|name| std::env::var_os(name))
    }

    /// Capture build and metadata target values from one injected source.
    pub(super) fn metadata_with_lookup(mut lookup: impl FnMut(&str) -> Option<OsString>) -> Self {
        let mut values = select_environment(CARGO_ENV_ALLOWLIST, &mut lookup);
        values.extend(select_environment(
            METADATA_TARGET_ENV_ALLOWLIST,
            &mut lookup,
        ));
        // cargo-llvm-cov selects its target with a command-line flag and
        // exports this variable, not CARGO_TARGET_DIR. Make nested metadata
        // resolve the actual instrumented parent target rather than a cold
        // default target from Cargo configuration.
        if let Some(coverage_target) = values.iter().find_map(|(name, value)| {
            (name.to_str() == Some("CARGO_LLVM_COV_TARGET_DIR")).then(|| value.clone())
        }) {
            values.retain(|(name, _)| name.to_str() != Some("CARGO_TARGET_DIR"));
            values.push((OsString::from("CARGO_TARGET_DIR"), coverage_target));
        }
        Self(values)
    }

    /// Select allow-listed values using an injectable lookup source.
    pub(super) fn from_lookup(
        allowlist: &[&str],
        lookup: impl FnMut(&str) -> Option<OsString>,
    ) -> Self {
        Self(select_environment(allowlist, lookup))
    }

    /// Add values prepared by the platform-specific linker discovery step.
    pub(super) fn extend(&mut self, values: impl IntoIterator<Item = (OsString, OsString)>) {
        self.0.extend(values);
    }

    /// Replace a command environment with the captured build-tool values.
    pub(super) fn apply(&self, command: &mut Command) {
        command.env_clear();
        command.envs(self.0.iter().map(|(name, value)| (name, value)));
    }

    #[cfg(test)]
    fn values(&self) -> &[(OsString, OsString)] {
        &self.0
    }
}

fn select_environment(
    allowlist: &[&str],
    mut lookup: impl FnMut(&str) -> Option<OsString>,
) -> Vec<(OsString, OsString)> {
    allowlist
        .iter()
        .filter_map(|name| lookup(name).map(|value| (OsString::from(name), value)))
        .collect()
}

#[cfg(test)]
#[path = "cargo_environment_tests.rs"]
mod tests;
