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
    ]);
    let environment = BuildEnvironment::from_lookup(|name| supplied.get(name).cloned());
    let mut command = Command::new("cargo");
    command.env("APPLICATION_SECRET", "must-not-reach-build-script");
    environment.apply(&mut command);
    let selected = command
        .get_envs()
        .filter_map(|(name, environment_value)| {
            environment_value.map(|selected_value| {
                (
                    name.to_str().expect("environment name is UTF-8"),
                    selected_value,
                )
            })
        })
        .collect::<HashMap<_, _>>();

    for (name, expected) in supplied
        .iter()
        .filter(|(name, _)| BUILD_ENV_ALLOWLIST.contains(name))
    {
        assert_eq!(selected.get(name), Some(&expected.as_os_str()));
    }
    assert!(!selected.contains_key("APPLICATION_SECRET"));
    assert_eq!(
        selected.get("CARGO_HOME"),
        Some(&OsStr::new("/home/agent/.cargo"))
    );
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
