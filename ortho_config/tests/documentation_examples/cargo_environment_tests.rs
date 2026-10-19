//! Specifications for the documentation Cargo environment boundary.

use super::{CARGO_ENV_ALLOWLIST, CargoEnvironment};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::process::Command;

const APPLICATION_SECRET: &str = "ORTHO_CONFIG_DOCUMENTATION_TEST_SECRET";
const CACHE_CREDENTIAL: &str = "ACTIONS_RUNTIME_TOKEN";
const EXPECTED_BUILD_VALUES: &[(&str, &str)] = &[
    ("CARGO_HOME", "/home/agent/.cargo"),
    ("SCCACHE_GHA_ENABLED", "true"),
    ("HOME", "/home/agent"),
    ("RUSTUP_HOME", "/home/agent/.rustup"),
    ("RUSTUP_TOOLCHAIN", "stable-x86_64-unknown-linux-gnu"),
    ("RUSTC", "/toolchain/bin/rustc"),
    ("RUSTC_WRAPPER", "/usr/bin/build-limits-rustc"),
    ("RUSTC_WORKSPACE_WRAPPER", "/usr/bin/workspace-wrapper"),
    ("RUSTDOC", "/toolchain/bin/rustdoc"),
    ("CARGO_BUILD_RUSTC_WRAPPER", "/config/rustc-wrapper"),
    (
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        "/config/workspace-wrapper",
    ),
    ("CARGO_BUILD_RUSTDOC", "/config/rustdoc"),
    ("CARGO_BUILD_JOBS", "4"),
    ("CARGO_INCREMENTAL", "0"),
    ("CARGO_LLVM_COV", "1"),
    ("CARGO_MAKEFLAGS", "--jobs=4"),
    ("MAKEFLAGS", "--jobs=4"),
    ("RUSTFLAGS", "-D warnings"),
    ("RUSTDOCFLAGS", "-D warnings"),
    ("CARGO_BUILD_RUSTFLAGS", "-D warnings"),
    ("CARGO_BUILD_RUSTDOCFLAGS", "-D warnings"),
    ("CARGO_ENCODED_RUSTFLAGS", "-D\u{1f}warnings"),
    ("CARGO_ENCODED_RUSTDOCFLAGS", "-D\u{1f}warnings"),
    ("LLVM_PROFILE_FILE", "/profiles/documentation-%p.profraw"),
    ("LLVM_PROFILE_MERGE_POOL_SIZE", "8"),
    ("BUILD_LIMITS_SOCKET", "/run/build-limits.sock"),
    ("XDG_RUNTIME_DIR", "/run/user/1000"),
];

#[test]
fn build_policy_keeps_toolchain_controls_and_excludes_application_values() {
    let mut supplied = EXPECTED_BUILD_VALUES
        .iter()
        .map(|(name, value)| (*name, OsString::from(value)))
        .collect::<HashMap<_, _>>();
    supplied.insert("CARGO_TARGET_DIR", OsString::from("/host/target"));
    supplied.insert(
        "CARGO_BUILD_TARGET_DIR",
        OsString::from("/configured/target"),
    );
    supplied.insert("XDG_CONFIG_HOME", OsString::from("/home/agent/.config"));
    supplied.insert(APPLICATION_SECRET, OsString::from("must-not-leak"));
    supplied.insert(CACHE_CREDENTIAL, OsString::from("must-not-leak"));

    let environment = CargoEnvironment::build_with_lookup(|name| supplied.get(name).cloned());
    let mut command = Command::new("cargo");
    command.env(APPLICATION_SECRET, "must-not-leak");
    environment.apply(&mut command);
    let selected = command
        .get_envs()
        .filter_map(|(name, environment_value)| {
            environment_value
                .map(|value| (name.to_str().expect("environment names are UTF-8"), value))
        })
        .collect::<HashMap<_, _>>();

    for (expected_name, expected_value) in EXPECTED_BUILD_VALUES {
        assert!(
            CARGO_ENV_ALLOWLIST.contains(expected_name),
            "{expected_name} should be an explicit build-policy entry"
        );
        assert_eq!(
            selected.get(expected_name).copied(),
            Some(OsStr::new(expected_value)),
            "build policy should preserve {expected_name}"
        );
    }
    assert!(!selected.contains_key("XDG_CONFIG_HOME"));
    assert!(!selected.contains_key(APPLICATION_SECRET));
    assert!(!selected.contains_key(CACHE_CREDENTIAL));
    assert!(!selected.contains_key("CARGO_TARGET_DIR"));
    assert!(!selected.contains_key("CARGO_BUILD_TARGET_DIR"));
}

#[rstest::rstest]
#[case::linux_wrapper("/tools/cargo-llvm-cov")]
#[case::windows_executable("/tools/cargo-llvm-cov.exe")]
fn nested_build_omits_coverage_cli_wrapper_but_keeps_coverage_policy(#[case] wrapper: &str) {
    let supplied = HashMap::from([
        ("RUSTC_WRAPPER", OsString::from(wrapper)),
        (
            "CARGO_BUILD_RUSTC_WRAPPER",
            OsString::from("/configured/sccache"),
        ),
        ("CARGO_LLVM_COV", OsString::from("1")),
        ("RUSTFLAGS", OsString::from("-C instrument-coverage")),
        (
            "LLVM_PROFILE_FILE",
            OsString::from("/profiles/documentation-%p.profraw"),
        ),
        ("CARGO_BUILD_JOBS", OsString::from("4")),
    ]);
    let environment = CargoEnvironment::build_with_lookup(|name| supplied.get(name).cloned());
    let selected = environment
        .values()
        .iter()
        .map(|(name, value)| (name.to_str().expect("name is UTF-8"), value.as_os_str()))
        .collect::<HashMap<_, _>>();

    assert!(!selected.contains_key("RUSTC_WRAPPER"));
    assert_eq!(
        selected.get("CARGO_BUILD_RUSTC_WRAPPER"),
        Some(&OsStr::new("/configured/sccache"))
    );
    assert_eq!(selected.get("CARGO_LLVM_COV"), Some(&OsStr::new("1")));
    assert_eq!(
        selected.get("RUSTFLAGS"),
        Some(&OsStr::new("-C instrument-coverage"))
    );
    assert_eq!(
        selected.get("LLVM_PROFILE_FILE"),
        Some(&OsStr::new("/profiles/documentation-%p.profraw"))
    );
    assert_eq!(selected.get("CARGO_BUILD_JOBS"), Some(&OsStr::new("4")));
}

#[test]
fn metadata_policy_alone_preserves_target_directory_overrides() {
    let supplied = HashMap::from([
        ("CARGO_HOME", "/home/agent/.cargo"),
        ("CARGO_TARGET_DIR", "/workspace/target"),
        ("CARGO_BUILD_TARGET_DIR", "/configured/target"),
    ]);
    let environment =
        CargoEnvironment::metadata_with_lookup(|name| supplied.get(name).map(OsString::from));
    let selected = environment
        .values()
        .iter()
        .map(|(name, value)| (name.to_str().expect("name is UTF-8"), value))
        .collect::<HashMap<_, _>>();

    assert_eq!(
        selected.get("CARGO_HOME").map(|value| value.as_os_str()),
        Some(OsStr::new("/home/agent/.cargo"))
    );
    assert_eq!(
        selected
            .get("CARGO_TARGET_DIR")
            .map(|value| value.as_os_str()),
        Some(OsStr::new("/workspace/target"))
    );
    assert_eq!(
        selected
            .get("CARGO_BUILD_TARGET_DIR")
            .map(|value| value.as_os_str()),
        Some(OsStr::new("/configured/target"))
    );
}

#[rstest::rstest]
#[case::normal_build(None, "/workspace/target")]
#[case::coverage_build(Some("/workspace/coverage-target"), "/workspace/coverage-target")]
fn metadata_selects_the_instrumented_parent_target(
    #[case] coverage_target: Option<&str>,
    #[case] expected_target: &str,
) {
    let mut supplied = HashMap::from([
        ("CARGO_TARGET_DIR", "/workspace/target"),
        (APPLICATION_SECRET, "must-not-leak"),
    ]);
    if let Some(target) = coverage_target {
        supplied.insert("CARGO_LLVM_COV_TARGET_DIR", target);
    }
    let environment =
        CargoEnvironment::metadata_with_lookup(|name| supplied.get(name).map(OsString::from));
    let selected = environment
        .values()
        .iter()
        .map(|(name, value)| (name.to_str().expect("name is UTF-8"), value.as_os_str()))
        .collect::<HashMap<_, _>>();
    assert_eq!(
        selected.get("CARGO_TARGET_DIR"),
        Some(&OsStr::new(expected_target))
    );
    assert!(!selected.contains_key(APPLICATION_SECRET));
}
