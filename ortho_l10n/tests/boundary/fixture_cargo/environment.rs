//! The closed environment a nested Cargo invocation is allowed to inherit.
//!
//! A boundary contract answers a question about a *stranger's* package. That
//! only means something if the child process sees the toolchain that built the
//! parent and nothing else from the developer's shell. So the list below is an
//! allow-list rather than a deny-list, and three kinds of variable are left out
//! on purpose:
//!
//! - anything that selects a target directory, such as `CARGO_TARGET_DIR` or
//!   `CARGO_BUILD_TARGET_DIR`, because the fixture owns its own target and a
//!   child sharing the parent's would contend on its fingerprints;
//! - anything that selects a Cargo configuration file or profile, such as
//!   `CARGO_BUILD_TARGET` and `CARGO_PROFILE_*`, because each fixture root
//!   renders its own manifest and must be resolved under Cargo's defaults;
//! - `CARGO` and `CARGO_MANIFEST_DIR`, which name the parent's own crate rather
//!   than a toolchain setting.
//!
//! Compiler flags and wrappers are *not* in that list. `RUSTFLAGS`,
//! `RUSTC_WRAPPER`, the `CARGO_BUILD_*` and `CARGO_ENCODED_*` spellings, and
//! `LLVM_PROFILE_FILE` are inherited deliberately: `make boundary` and `make
//! test` both build with `RUSTFLAGS="-D warnings"`, and the child has to be
//! held to the same policy as the parent, or a fixture that compiles with a
//! warning would pass a run the parent gate fails. The coverage wrapper is the
//! one exception, and [`remove_coverage_cli_wrapper`] explains why.
//!
//! The list mirrors the closed policy the documentation-example harness already
//! applies, so both nested-Cargo suites in this repository agree about what a
//! build child may see.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

/// Longest a single nested Cargo invocation may run.
///
/// A recent gate observed a nested metadata command wait nearly four minutes on
/// the shared package-cache lock, and other children hit their 300-second
/// deadlines. This bound leaves two more minutes for a queued command to run
/// while still limiting a stuck child.
pub(crate) const CARGO_DEADLINE: Duration = Duration::from_secs(420);

/// Toolchain and platform variables a nested Cargo invocation may inherit.
///
/// Compiler admission (`BUILD_LIMITS_*`, `RUSTC_WRAPPER`), the Cargo and rustup
/// homes, the system link path, and the MSVC discovery variables are here
/// because a build child cannot work without them. The compiler flags and
/// wrappers are here so the child is built under the same `-D warnings` policy
/// as the parent. Nothing that selects a target directory, a configuration
/// file, or a profile is; the module doc lists those by name.
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
