//! Closed build-tool environment policy for documentation tests.
//!
//! Cargo retains the user's configured shared cache and Cargo configuration.
//! Only named toolchain, compiler-admission, and platform variables cross this
//! boundary; application processes use the narrower policy in `workspace`.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

pub(crate) const CARGO_ENV_ALLOWLIST: &[&str] = &[
    "BUILD_LIMITS_COMPILERS_SLICE",
    "BUILD_LIMITS_HEARTBEAT_SECS",
    "BUILD_LIMITS_RUSTUP",
    "BUILD_LIMITS_SOCKET",
    "BUILD_LIMITS_UNAVAILABLE_SECS",
    "AR",
    "CC",
    "CFLAGS",
    "CXX",
    "CXXFLAGS",
    "CARGO_BUILD_JOBS",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTDOC",
    "CARGO_BUILD_RUSTDOCFLAGS",
    "CARGO_BUILD_RUSTFLAGS",
    "CARGO_ENCODED_RUSTDOCFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_HOME",
    "CARGO_HTTP_MULTIPLEXING",
    "CARGO_HTTP_TIMEOUT",
    "CARGO_INCREMENTAL",
    "CARGO_LLVM_COV",
    "CARGO_MAKEFLAGS",
    "CARGO_NET_GIT_FETCH_WITH_CLI",
    "CARGO_NET_OFFLINE",
    "CARGO_NET_RETRY",
    "DEVELOPER_DIR",
    "HOME",
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "LDFLAGS",
    "LLVM_PROFILE_FILE",
    "LLVM_PROFILE_MERGE_POOL_SIZE",
    "MAKEFLAGS",
    "MACOSX_DEPLOYMENT_TARGET",
    "OPENSSL_DIR",
    "OPENSSL_INCLUDE_DIR",
    "OPENSSL_LIB_DIR",
    "PATH",
    "PKG_CONFIG",
    "PKG_CONFIG_PATH",
    "PKG_CONFIG_SYSROOT_DIR",
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
    "SDKROOT",
    "SSL_CERT_DIR",
    "SSL_CERT_FILE",
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

const METADATA_TARGET_ENV_ALLOWLIST: &[&str] = &[
    "CARGO_BUILD_TARGET_DIR",
    "CARGO_TARGET_DIR",
    "CARGO_LLVM_COV_TARGET_DIR",
];

/// A selected environment for a Cargo or host build-tool command.
pub(crate) struct CargoEnvironment(Vec<(OsString, OsString)>);

impl CargoEnvironment {
    /// Capture only the closed toolchain, wrapper, and platform allow-list.
    pub(crate) fn build() -> Self {
        Self::build_with_lookup(|name| std::env::var_os(name))
    }

    /// Select the nested build environment without forwarding cargo-llvm-cov
    /// as though its CLI were a rustc wrapper.
    pub(crate) fn build_with_lookup(lookup: impl FnMut(&str) -> Option<OsString>) -> Self {
        let mut environment = Self::from_lookup(CARGO_ENV_ALLOWLIST, lookup);
        remove_coverage_cli_wrapper(&mut environment.0);
        environment
    }

    /// Capture the build environment plus caller target paths for metadata.
    pub(crate) fn metadata() -> Self {
        Self::metadata_with_lookup(|name| std::env::var_os(name))
    }

    /// Capture build and metadata target values from one injected source.
    pub(crate) fn metadata_with_lookup(mut lookup: impl FnMut(&str) -> Option<OsString>) -> Self {
        let mut environment = Self::from_lookup(CARGO_ENV_ALLOWLIST, &mut lookup);
        environment.0.extend(select_environment(
            METADATA_TARGET_ENV_ALLOWLIST,
            &mut lookup,
        ));
        // cargo-llvm-cov selects its target with a command-line flag and
        // exports this variable, not CARGO_TARGET_DIR. Make nested metadata
        // resolve the actual instrumented parent target rather than a cold
        // default target from Cargo configuration.
        if let Some(coverage_target) = environment.0.iter().find_map(|(name, value)| {
            (name.to_str() == Some("CARGO_LLVM_COV_TARGET_DIR")).then(|| value.clone())
        }) {
            environment
                .0
                .retain(|(name, _)| name.to_str() != Some("CARGO_TARGET_DIR"));
            environment
                .0
                .push((OsString::from("CARGO_TARGET_DIR"), coverage_target));
        }
        remove_coverage_cli_wrapper(&mut environment.0);
        environment
    }

    /// Select allow-listed values using an injectable lookup source.
    pub(crate) fn from_lookup(
        allowlist: &[&str],
        lookup: impl FnMut(&str) -> Option<OsString>,
    ) -> Self {
        Self(select_environment(allowlist, lookup))
    }

    /// Replace a command environment with the captured build-tool values.
    pub(crate) fn apply(&self, command: &mut Command) {
        command.env_clear();
        command.envs(self.0.iter().map(|(name, value)| (name, value)));
    }
}

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

fn select_environment(
    allowlist: &[&str],
    mut lookup: impl FnMut(&str) -> Option<OsString>,
) -> Vec<(OsString, OsString)> {
    allowlist
        .iter()
        .filter_map(|name| lookup(name).map(|value| (OsString::from(name), value)))
        .collect()
}
