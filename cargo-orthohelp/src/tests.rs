//! Tests for ephemeral bridge crate orchestration.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use tempfile::tempdir;

use super::*;
use crate::cache::{CACHE_DIRECTORY_DIGEST_BYTES, CACHE_KEY_VERSION_PREFIX};

fn dummy_paths() -> BridgePaths {
    BridgePaths {
        bridge_dir: Utf8PathBuf::from("/tmp/bridge"),
        manifest_path: Utf8PathBuf::from("/tmp/bridge/Cargo.toml"),
        target_dir: Utf8PathBuf::from("/tmp/bridge/target"),
        binary_name: "orthohelp_bridge_test".to_owned(),
    }
}

#[test]
fn build_bridge_command_applies_the_selected_build_environment() {
    let supplied = HashMap::from([
        ("RUSTC_WRAPPER", OsString::from("/tools/build-limits-rustc")),
        ("RUSTDOC", OsString::from("/toolchain/bin/rustdoc")),
        ("CARGO_BUILD_JOBS", OsString::from("4")),
        ("APPLICATION_SECRET", OsString::from("must-not-leak")),
    ]);
    let environment = BuildEnvironment::from_lookup(|name| supplied.get(name).cloned());
    let command = build_bridge_command(&dummy_paths(), &environment);
    let arguments = command.get_args().collect::<Vec<_>>();
    assert!(
        arguments
            .iter()
            .any(|argument| *argument == OsStr::new("--message-format=json")),
        "the bridge build must keep rendered diagnostics inside the bounded JSON stream: {arguments:?}"
    );
    assert!(arguments.windows(2).any(|pair| {
        pair.first()
            .copied()
            .is_some_and(|argument| argument == OsStr::new("--bin"))
            && pair
                .get(1)
                .copied()
                .is_some_and(|argument| argument == OsStr::new("orthohelp_bridge_test"))
    }));
    let selected = command
        .get_envs()
        .filter_map(|(name, environment_value)| {
            environment_value.map(|selected_value| (name, selected_value))
        })
        .collect::<HashMap<_, _>>();

    assert_eq!(
        selected.get(OsStr::new("RUSTC_WRAPPER")),
        Some(&OsStr::new("/tools/build-limits-rustc"))
    );
    assert_eq!(
        selected.get(OsStr::new("RUSTDOC")),
        Some(&OsStr::new("/toolchain/bin/rustdoc"))
    );
    assert_eq!(
        selected.get(OsStr::new("CARGO_BUILD_JOBS")),
        Some(&OsStr::new("4"))
    );
    assert!(!selected.contains_key(OsStr::new("APPLICATION_SECRET")));
}

#[test]
fn generated_binary_names_are_private_to_each_invocation() {
    let package_root = camino::Utf8Path::new("/workspace/application");
    let invocation_root = camino::Utf8Path::new("/cache/invocation-a");
    let other_invocation_root = camino::Utf8Path::new("/cache/invocation-b");
    let binary_name = workspace::bridge_binary_target_name(package_root, invocation_root);
    let other_package_name = workspace::bridge_binary_target_name(
        camino::Utf8Path::new("/workspace/other-application"),
        invocation_root,
    );
    let other_invocation_name =
        workspace::bridge_binary_target_name(package_root, other_invocation_root);

    assert!(binary_name.starts_with("orthohelp_bridge_workspace_"));
    assert_ne!(binary_name, "orthohelp_bridge");
    assert_eq!(
        binary_name,
        workspace::bridge_binary_target_name(package_root, invocation_root)
    );
    assert_ne!(binary_name, other_package_name);
    assert_ne!(binary_name, other_invocation_name);
}

#[test]
fn bridge_manifest_disables_the_default_binary_target() {
    let temporary = tempdir().expect("temporary workspace");
    let bridge_dir =
        Utf8PathBuf::from_path_buf(temporary.path().to_path_buf()).expect("UTF-8 temporary path");
    let paths = BridgePaths {
        manifest_path: bridge_dir.join("Cargo.toml"),
        bridge_dir: bridge_dir.clone(),
        target_dir: bridge_dir.join("target"),
        binary_name: "orthohelp_bridge_workspace_fixture".to_owned(),
    };
    let config = BridgeConfig {
        workspace_root: bridge_dir.clone(),
        package_root: bridge_dir.clone(),
        package_name: "example".to_owned(),
        root_type: "example::Config".to_owned(),
        ortho_config_dependency: OrthoConfigDependency {
            requirement: "0.9.0".to_owned(),
            path: None,
        },
    };

    ensure_bridge_layout(&paths).expect("create generated crate layout");
    write_bridge_manifest(&config, &paths).expect("write generated manifest");
    let directory = Dir::open_ambient_dir(&bridge_dir, ambient_authority())
        .expect("open generated crate directory");
    let manifest = directory
        .read_to_string("Cargo.toml")
        .expect("read generated manifest");

    assert!(manifest.contains("autobins = false"));
    assert!(manifest.contains(
        "[[bin]]\nname = \"orthohelp_bridge_workspace_fixture\"\npath = \"src/main.rs\""
    ));
}

/// Longest package name among the fixtures the behavioural suite builds.
///
/// It is not a guess: the failing crate is the one whose linker-input path
/// crosses `MAX_PATH`, so it is the crate a regression would hit first.
const LONGEST_FIXTURE_PACKAGE: &str = "orthohelp_policy_adoption_fixture";

/// Windows still applies a 260-character `MAX_PATH` to the file names
/// `link.exe` opens, even where long paths are enabled for other APIs.
const WINDOWS_MAX_PATH: usize = 260;

/// Depth of the workspace path a hosted Windows runner checks the repository
/// out to, `D:\a\<repository>\<repository>`.
///
/// The backslashes are doubled because this suite runs on Linux, where a
/// single one is a directory separator that `Utf8Path` would collapse. Only
/// the character count matters here, and `\` and `/` cost the same, so the
/// measurement is the one Windows would make.
const RUNNER_WORKSPACE: &str = r"D:\\a\\ortho-config\\ortho-config";

#[test]
fn link_input_paths_fit_in_the_windows_path_limit() {
    // From cargo 1.100 every artefact is written under
    // `build/<crate>/<hash>/out/` instead of `deps/`, which put the longest
    // fixture's `.rlib` at 262 characters on a hosted runner and made
    // `link.exe` fail with LNK1104 while building the bridge. Rebuilding the
    // path here keeps the next lengthening of any component, the crate name,
    // the cache key, or the layout, inside this suite rather than on Windows.
    //
    // `<crate>/<hash>` mirrors cargo's own directory shape, where both hashes
    // are 16 digits; the `.rlib` name repeats the same two terms.
    let link_input = Utf8Path::new(RUNNER_WORKSPACE)
        .join("target")
        .join("orthohelp-bdd")
        .join(".tmpXXXXXX")
        .join("orthohelp")
        .join("x".repeat(CACHE_KEY_VERSION_PREFIX.len() + CACHE_DIRECTORY_DIGEST_BYTES * 2))
        .join("target")
        .join("debug")
        .join("build")
        .join(LONGEST_FIXTURE_PACKAGE)
        .join("0123456789abcdef")
        .join("out")
        .join(format!(
            "lib{LONGEST_FIXTURE_PACKAGE}-0123456789abcdef.rlib"
        ));

    assert!(
        link_input.as_str().len() < WINDOWS_MAX_PATH,
        "linker input is {} characters, but must be shorter than the \
         {WINDOWS_MAX_PATH}-character Windows limit: {link_input}",
        link_input.as_str().len()
    );
}
