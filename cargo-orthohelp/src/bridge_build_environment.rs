//! Select build-tool inputs for the generated Cargo bridge build.
//!
//! Cargo still reads project and home configuration and owns its shared cache.
//! This policy forwards approved build controls without exposing application
//! values or credential-bearing proxy and registry URLs.
//! It is limited to bridge compilation; generated programs keep their own
//! runtime environment policy.

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

/// Exact environment names forwarded to the nested bridge build tool.
const BUILD_ENV_ALLOWLIST: &[&str] = &[
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
    "CARGO_HTTP_PROXY",
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
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "LDFLAGS",
    "LLVM_PROFILE_FILE",
    "LLVM_PROFILE_MERGE_POOL_SIZE",
    "MAKEFLAGS",
    "MACOSX_DEPLOYMENT_TARGET",
    "NO_PROXY",
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
    "ALL_PROXY",
    "all_proxy",
    "http_proxy",
    "https_proxy",
    "no_proxy",
];

/// Build-tool variables for one nested Cargo invocation.
pub(super) struct BuildEnvironment(Vec<(OsString, OsString)>);

impl BuildEnvironment {
    /// Capture only named toolchain controls from the current process.
    pub(super) fn from_process() -> Self {
        Self::from_values(std::env::vars_os())
    }

    /// Select named environment values from an injected source for tests.
    #[cfg(test)]
    pub(super) fn from_lookup(mut lookup: impl FnMut(&str) -> Option<OsString>) -> Self {
        Self::from_values(
            BUILD_ENV_ALLOWLIST
                .iter()
                .filter_map(|name| lookup(name).map(|value| (OsString::from(*name), value)))
                .collect::<Vec<_>>(),
        )
    }

    /// Filter an injected or process environment to safe build-tool inputs.
    fn from_values(environment_values: impl IntoIterator<Item = (OsString, OsString)>) -> Self {
        let mut selected_values = environment_values
            .into_iter()
            .filter(|(name, _)| is_allowed_name(name))
            .filter(|(name, value)| is_safe_value(name, value))
            .collect::<Vec<_>>();
        remove_coverage_cli_wrapper(&mut selected_values);
        Self(selected_values)
    }

    /// Replace the child environment while preserving Cargo's config search.
    pub(super) fn apply(&self, command: &mut Command) {
        command.env_clear();
        command.envs(self.0.iter().map(|(name, value)| (name, value)));
    }
}

/// Check the fixed allow-list and narrow Cargo target/registry settings.
fn is_allowed_name(environment_name: &OsStr) -> bool {
    let Some(name) = comparable_name(environment_name) else {
        return false;
    };
    is_listed_name(name.as_ref())
        || is_target_build_override(name.as_ref())
        || is_registry_index(name.as_ref())
}

/// Match host environment keys without changing the spelling forwarded to Cargo.
fn comparable_name(environment_name: &OsStr) -> Option<Cow<'_, str>> {
    let name = environment_name.to_str()?;
    #[cfg(windows)]
    {
        Some(Cow::Owned(name.to_ascii_uppercase()))
    }
    #[cfg(not(windows))]
    {
        Some(Cow::Borrowed(name))
    }
}

/// Compare one normalized key with the fixed environment allow-list.
fn is_listed_name(name: &str) -> bool {
    BUILD_ENV_ALLOWLIST.iter().any(|allowed| {
        comparable_name(OsStr::new(allowed)).is_some_and(|candidate| candidate.as_ref() == name)
    })
}

/// Recognize only target linker and rustflags variables.
fn is_target_build_override(name: &str) -> bool {
    let Some(scoped_setting) = name.strip_prefix("CARGO_TARGET_") else {
        return false;
    };
    let Some((scope, setting)) = scoped_setting.rsplit_once('_') else {
        return false;
    };
    is_valid_cargo_scope(scope) && matches!(setting, "LINKER" | "RUSTFLAGS")
}

/// Recognize registry index URLs while excluding credential variables.
fn is_registry_index(name: &str) -> bool {
    let Some(registry_scope) = name.strip_prefix("CARGO_REGISTRIES_") else {
        return false;
    };
    let Some(scope) = registry_scope.strip_suffix("_INDEX") else {
        return false;
    };
    is_valid_cargo_scope(scope)
}

/// Accept Cargo's uppercase scope spelling and embedded underscores.
fn is_valid_cargo_scope(scope: &str) -> bool {
    !scope.is_empty()
        && scope
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Reject non-UTF-8 and credential-bearing proxy or index values.
fn is_safe_value(environment_name: &OsStr, value: &OsStr) -> bool {
    let Some(raw_name) = environment_name.to_str() else {
        return false;
    };
    let Some(comparable) = comparable_name(environment_name) else {
        return false;
    };
    let name = comparable.as_ref();
    let may_contain_credentials = matches!(
        name,
        "ALL_PROXY"
            | "all_proxy"
            | "CARGO_HTTP_PROXY"
            | "HTTP_PROXY"
            | "http_proxy"
            | "HTTPS_PROXY"
            | "https_proxy"
            | "NO_PROXY"
            | "no_proxy"
    ) || is_registry_index(name);
    if !may_contain_credentials {
        return true;
    }
    let is_safe = value.to_str().is_some_and(|text| {
        !text
            .chars()
            .any(|character| matches!(character, '@' | '?' | '#'))
    });
    if !is_safe {
        super::bridge_build::telemetry::credential_bearing_setting_omitted(raw_name);
    }
    is_safe
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
