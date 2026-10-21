//! The closed environment a nested Cargo invocation is allowed to inherit.
//!
//! A boundary contract answers a question about a *stranger's* package. That
//! only means something if the child process sees the toolchain that built the
//! parent and nothing else from the developer's shell: an inherited
//! `CARGO_TARGET_DIR`, a `RUSTFLAGS` override, or a stray `RUSTC_WRAPPER` from a
//! coverage run would each make the child build something other than what the
//! contract claims to measure.
//!
//! The list mirrors the closed policy the documentation-example harness already
//! applies, so both nested-Cargo suites in this repository agree about what a
//! build child may see.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

/// Longest a single nested Cargo invocation may run.
///
/// The contracts resolve small consumer roots against the shared package cache;
/// the slowest observed run is tens of seconds, not minutes. The bound exists so
/// a child blocked on the package-cache lock is attributed to its own contract
/// instead of consuming the whole test allowance silently.
pub(crate) const CARGO_DEADLINE: Duration = Duration::from_secs(300);

/// Toolchain and platform variables a nested Cargo invocation may inherit.
///
/// Compiler admission (`BUILD_LIMITS_*`, `RUSTC_WRAPPER`), the Cargo and rustup
/// homes, the system link path, and the MSVC discovery variables are here
/// because a build child cannot work without them. Nothing that selects a
/// target directory, a configuration file, or a profile is.
pub(crate) const INHERITED_VARIABLES: &[&str] = &[
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
    "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER",
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

/// The Cargo executable the parent test binary was launched with.
///
/// Reading `CARGO` rather than naming `cargo` keeps the child on the exact
/// binary Cargo chose for the parent, which matters when several toolchains are
/// installed.
pub(crate) fn cargo_binary() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

/// Collect the allow-listed toolchain variables from the current process.
///
/// The coverage wrapper is removed first. `cargo llvm-cov` exports itself as
/// `RUSTC_WRAPPER`; handing that to a nested build would instrument a package
/// the contract never asked to measure, and on some runners would fail outright.
pub(crate) fn inherited_toolchain() -> Vec<(OsString, OsString)> {
    let mut values = INHERITED_VARIABLES
        .iter()
        .filter_map(|name| std::env::var_os(name).map(|value| (OsString::from(name), value)))
        .collect::<Vec<_>>();
    remove_coverage_cli_wrapper(&mut values);
    values
}

/// Drop `RUSTC_WRAPPER` when it is `cargo llvm-cov`'s CLI wrapper.
fn remove_coverage_cli_wrapper(values: &mut Vec<(OsString, OsString)>) {
    let is_coverage_cli = values
        .iter()
        .find(|(name, _)| name == "RUSTC_WRAPPER")
        .is_some_and(|(_, value)| {
            Path::new(value)
                .file_stem()
                .is_some_and(|stem| stem == "cargo-llvm-cov")
        });
    if is_coverage_cli {
        values.retain(|(name, _)| name != "RUSTC_WRAPPER");
    }
}
