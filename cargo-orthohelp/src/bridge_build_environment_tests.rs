//! Specifications for the nested bridge build environment.

use super::{BUILD_ENV_ALLOWLIST, BuildEnvironment};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::process::Command;

#[test]
fn build_policy_preserves_toolchain_inputs_and_excludes_application_values() {
    let supplied = HashMap::from([
        ("CARGO_HOME", OsString::from("/home/agent/.cargo")),
        ("CARGO_BUILD_JOBS", OsString::from("4")),
        ("CARGO_NET_OFFLINE", OsString::from("true")),
        (
            "CARGO_HTTP_PROXY",
            OsString::from("http://cargo-proxy.example:3128"),
        ),
        ("CC", OsString::from("clang")),
        (
            "PKG_CONFIG_PATH",
            OsString::from("/toolchain/lib/pkgconfig"),
        ),
        ("INCLUDE", OsString::from(r"C:\Visual Studio\include")),
        (
            "LIB",
            OsString::from(r"C:\Visual Studio\lib;C:\Windows Kits\um\x64"),
        ),
        ("LIBPATH", OsString::from(r"C:\Visual Studio\references")),
        (
            "CARGO_BUILD_RUSTC_WRAPPER",
            OsString::from("/tools/sccache"),
        ),
        ("RUSTDOC", OsString::from("/toolchain/bin/rustdoc")),
        ("RUSTC_WRAPPER", OsString::from("/tools/build-limits-rustc")),
        ("RUSTFLAGS", OsString::from("-D warnings")),
        ("CARGO_LLVM_COV", OsString::from("1")),
        (
            "LLVM_PROFILE_FILE",
            OsString::from("/profiles/bridge-%p.profraw"),
        ),
        (
            "APPLICATION_SECRET",
            OsString::from("must-not-reach-build-script"),
        ),
        (
            "CARGO_REGISTRY_TOKEN",
            OsString::from("must-not-reach-build-script"),
        ),
        (
            "SSH_AUTH_SOCK",
            OsString::from("/run/user/1000/ssh-agent.sock"),
        ),
        ("HTTP_PROXY", OsString::from("http://proxy.example:3128")),
        ("NO_PROXY", OsString::from("localhost,127.0.0.1")),
        ("http_proxy", OsString::from("http://proxy.example:8080")),
        (
            "HTTPS_PROXY",
            OsString::from("https://user:password@proxy.example"),
        ),
    ]);
    let environment = BuildEnvironment::from_lookup(|name| supplied.get(name).cloned());
    let selected = selected_environment(&environment);

    for (name, expected) in supplied.iter().filter(|(name, value)| {
        BUILD_ENV_ALLOWLIST.contains(name) && super::is_safe_value(OsStr::new(name), value)
    }) {
        assert_eq!(
            selected.get(*name).map(OsString::as_os_str),
            Some(expected.as_os_str())
        );
    }
    assert_selected_environment_boundaries(&selected);
}

fn assert_selected_environment_boundaries(selected: &HashMap<String, OsString>) {
    assert!(!selected.contains_key("APPLICATION_SECRET"));
    assert!(!selected.contains_key("CARGO_REGISTRY_TOKEN"));
    assert!(!selected.contains_key("HTTPS_PROXY"));
    assert!(!selected.contains_key("SSH_AUTH_SOCK"));
    assert_eq!(
        selected.get("http_proxy").map(OsString::as_os_str),
        Some(OsStr::new("http://proxy.example:8080"))
    );
    assert_eq!(
        selected.get("CARGO_HOME").map(OsString::as_os_str),
        Some(OsStr::new("/home/agent/.cargo"))
    );
}

fn selected_environment(environment: &BuildEnvironment) -> HashMap<String, OsString> {
    let mut command = Command::new("cargo");
    command.env("APPLICATION_SECRET", "must-not-reach-build-script");
    environment.apply(&mut command);
    command
        .get_envs()
        .filter_map(|(name, environment_value)| {
            environment_value
                .map(|value| (name.to_string_lossy().into_owned(), value.to_os_string()))
        })
        .collect()
}

#[test]
fn dynamic_cargo_build_inputs_are_narrow_and_credential_free() {
    let supplied = HashMap::from([
        (
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER",
            OsString::from("clang"),
        ),
        (
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
            OsString::from("-D warnings"),
        ),
        (
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_TOKEN",
            OsString::from("must-not-reach-build-script"),
        ),
        (
            "CARGO_REGISTRIES_INTERNAL_INDEX",
            OsString::from("sparse+https://registry.example/index/"),
        ),
        (
            "CARGO_REGISTRIES_INTERNAL_TOKEN",
            OsString::from("must-not-reach-build-script"),
        ),
        (
            "CARGO_HTTP_PROXY",
            OsString::from("http://proxy.example:8080?token=secret"),
        ),
    ]);
    let environment = BuildEnvironment::from_values(
        supplied
            .into_iter()
            .map(|(name, value)| (OsString::from(name), value)),
    );
    let selected = environment
        .0
        .iter()
        .map(|(name, value)| (name.to_str().expect("name is UTF-8"), value.as_os_str()))
        .collect::<HashMap<_, _>>();

    assert_eq!(
        selected.get("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"),
        Some(&OsStr::new("clang"))
    );
    assert_eq!(
        selected.get("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS"),
        Some(&OsStr::new("-D warnings"))
    );
    assert_eq!(
        selected.get("CARGO_REGISTRIES_INTERNAL_INDEX"),
        Some(&OsStr::new("sparse+https://registry.example/index/"))
    );
    assert!(!selected.contains_key("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_TOKEN"));
    assert!(!selected.contains_key("CARGO_REGISTRIES_INTERNAL_TOKEN"));
    assert!(!selected.contains_key("CARGO_HTTP_PROXY"));
}

#[cfg(windows)]
#[test]
fn windows_environment_names_are_matched_without_exposing_credentials() {
    let values = [
        ("Path", OsString::from(r"C:\toolchain\bin")),
        ("SystemRoot", OsString::from(r"C:\Windows")),
        (
            "Https_Proxy",
            OsString::from("https://user:secret@proxy.example"),
        ),
        (
            "Cargo_Target_x86_64_pc_windows_msvc_Linker",
            OsString::from("clang-cl"),
        ),
        (
            "Cargo_Registries_Internal_Index",
            OsString::from("sparse+https://registry.example/index/"),
        ),
        (
            "Cargo_Registries_Internal_Token",
            OsString::from("must-not-reach-build-script"),
        ),
    ]
    .into_iter()
    .map(|(name, value)| (OsString::from(name), value));
    let environment = BuildEnvironment::from_values(values);
    let selected = environment
        .0
        .iter()
        .map(|(name, value)| (name.to_string_lossy().into_owned(), value.as_os_str()))
        .collect::<HashMap<_, _>>();

    assert_eq!(selected.get("Path"), Some(&OsStr::new(r"C:\toolchain\bin")));
    assert_eq!(selected.get("SystemRoot"), Some(&OsStr::new(r"C:\Windows")));
    assert_eq!(
        selected.get("Cargo_Target_x86_64_pc_windows_msvc_Linker"),
        Some(&OsStr::new("clang-cl"))
    );
    assert_eq!(
        selected.get("Cargo_Registries_Internal_Index"),
        Some(&OsStr::new("sparse+https://registry.example/index/"))
    );
    assert!(!selected.contains_key("Https_Proxy"));
    assert!(!selected.contains_key("Cargo_Registries_Internal_Token"));
}

#[rstest::rstest]
#[case::linux_cli("/tools/cargo-llvm-cov")]
#[case::windows_cli("/tools/cargo-llvm-cov.exe")]
fn coverage_cli_is_not_used_as_the_nested_rustc_wrapper(#[case] wrapper: &str) {
    let supplied = HashMap::from([
        ("RUSTC_WRAPPER", OsString::from(wrapper)),
        (
            "CARGO_BUILD_RUSTC_WRAPPER",
            OsString::from("/tools/configured-wrapper"),
        ),
        ("CARGO_LLVM_COV", OsString::from("1")),
        ("RUSTFLAGS", OsString::from("-C instrument-coverage")),
    ]);
    let environment = BuildEnvironment::from_lookup(|name| supplied.get(name).cloned());
    let selected = environment
        .0
        .iter()
        .map(|(name, value)| (name.to_str().expect("name is UTF-8"), value.as_os_str()))
        .collect::<HashMap<_, _>>();

    assert!(!selected.contains_key("RUSTC_WRAPPER"));
    assert_eq!(
        selected.get("CARGO_BUILD_RUSTC_WRAPPER"),
        Some(&OsStr::new("/tools/configured-wrapper"))
    );
    assert_eq!(selected.get("CARGO_LLVM_COV"), Some(&OsStr::new("1")));
    assert_eq!(
        selected.get("RUSTFLAGS"),
        Some(&OsStr::new("-C instrument-coverage"))
    );
}
