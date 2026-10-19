//! Select build-tool inputs for the generated Cargo bridge build.
//!
//! Cargo still reads project and home configuration and owns its shared cache.
//! This policy forwards compiler controls without exposing application values.
//! It is limited to bridge compilation; generated programs keep their own
//! runtime environment policy.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

/// Environment names forwarded to the nested bridge build tool.
const BUILD_ENV_ALLOWLIST: &[&str] = &[
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
    "TEMP",
    "TMP",
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

/// Build-tool variables for one nested Cargo invocation.
pub(super) struct BuildEnvironment(Vec<(OsString, OsString)>);

impl BuildEnvironment {
    /// Capture only named toolchain controls from the current process.
    pub(super) fn from_process() -> Self {
        Self::from_lookup(|name| std::env::var_os(name))
    }

    /// Select the build policy from an injected source for deterministic tests.
    pub(super) fn from_lookup(mut lookup: impl FnMut(&str) -> Option<OsString>) -> Self {
        let mut values = BUILD_ENV_ALLOWLIST
            .iter()
            .filter_map(|name| lookup(name).map(|value| (OsString::from(*name), value)))
            .collect::<Vec<_>>();
        remove_coverage_cli_wrapper(&mut values);
        Self(values)
    }

    /// Replace the child environment while preserving Cargo's config search.
    pub(super) fn apply(&self, command: &mut Command) {
        command.env_clear();
        command.envs(self.0.iter().map(|(name, value)| (name, value)));
    }
}

/// Avoid treating the coverage CLI itself as Cargo's compiler wrapper.
fn remove_coverage_cli_wrapper(values: &mut Vec<(OsString, OsString)>) {
    let is_coverage_cli = values
        .iter()
        .find(|(name, _)| name == "RUSTC_WRAPPER")
        .is_some_and(|(_, value)| {
            Path::new(value)
                .file_stem()
                .is_some_and(|name| name == "cargo-llvm-cov")
        });
    if is_coverage_cli {
        values.retain(|(name, _)| name != "RUSTC_WRAPPER");
    }
}

#[cfg(test)]
#[path = "bridge_build_environment_tests.rs"]
mod tests;
