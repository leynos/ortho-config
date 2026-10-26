//! Compile-time UI tests for `cargo-orthohelp`.
//!
//! These tests use `trybuild` to assert that code patterns which should be
//! rejected by the compiler (e.g. ignoring `#[must_use]` return values) do in
//! fact fail to compile.

use std::process::Command;

const TRYBUILD_CHILD_ENV: &str = "ORTHOHELP_TRYBUILD_CHILD";
const RUSTFLAGS_SEPARATOR: char = '\u{1f}';
const DEV_FAST_RUSTFLAGS: [&str; 5] = [
    "-Zthreads=8",
    "-Zcodegen-backend=cranelift",
    "--cfg=ortho_config_dev_fast",
    "--check-cfg=cfg(ortho_config_dev_fast)",
    "-Clink-arg=-fuse-ld=mold",
];

fn trybuild_rustflags(package_root: &str) -> String {
    let encoded_flags = std::env::var("CARGO_ENCODED_RUSTFLAGS").ok();
    let rustflags = std::env::var("RUSTFLAGS").ok();
    // Linux's target table replaces the build-wide value; other hosts use
    // the workspace's `[build]` table.
    let target_rustflags = if cfg!(target_os = "linux") {
        Some(configured_development_rustflags())
    } else {
        None
    };
    let build_rustflags = std::env::var("CARGO_BUILD_RUSTFLAGS").ok();
    let flags = selected_rustflags(
        encoded_flags.as_deref(),
        rustflags.as_deref(),
        target_rustflags.as_deref(),
        build_rustflags.as_deref(),
    );
    let is_coverage_build = has_coverage_instrumentation(&flags);
    compose_trybuild_rustflags(&flags, is_coverage_build, package_root)
}

fn selected_rustflags(
    encoded_flags: Option<&str>,
    rustflags: Option<&str>,
    target_rustflags: Option<&[String]>,
    build_rustflags: Option<&str>,
) -> Vec<String> {
    if let Some(flags) = encoded_flags {
        return split_encoded_rustflags(flags);
    }
    if let Some(flags) = rustflags {
        return flags.split_whitespace().map(str::to_owned).collect();
    }
    // Cargo chooses matching target flags before `[build].rustflags`, even
    // when the latter comes from `CARGO_BUILD_RUSTFLAGS`.
    if let Some(flags) = target_rustflags {
        return flags.to_vec();
    }
    if let Some(flags) = build_rustflags {
        return flags.split_whitespace().map(str::to_owned).collect();
    }
    configured_development_rustflags()
}

fn split_encoded_rustflags(flags: &str) -> Vec<String> {
    flags
        .split(RUSTFLAGS_SEPARATOR)
        .filter(|flag| !flag.is_empty())
        .map(str::to_owned)
        .collect()
}

fn configured_development_rustflags() -> Vec<String> {
    let flag_count = if cfg!(target_os = "linux") { 5 } else { 4 };
    DEV_FAST_RUSTFLAGS
        .iter()
        .take(flag_count)
        .map(|flag| (*flag).to_owned())
        .collect()
}

fn has_coverage_instrumentation(flags: &[String]) -> bool {
    flags.iter().any(|flag| flag == "-Cinstrument-coverage")
        || flags
            .windows(2)
            .any(|pair| pair == ["-C", "instrument-coverage"])
}

fn compose_trybuild_rustflags(
    selected_flags: &[String],
    is_coverage_build: bool,
    package_root: &str,
) -> String {
    let mut flags = selected_flags.to_vec();
    if is_coverage_build {
        flags.retain(|flag| !DEV_FAST_RUSTFLAGS.contains(&flag.as_str()));
    }
    flags.extend(
        [
            "--cfg",
            "trybuild",
            "--verbose",
            "-A",
            "dead_code",
            "--diagnostic-width=140",
        ]
        .map(str::to_owned),
    );

    // Nested trybuild Cargo builds need the requested LLVM instrumentation.
    // Keep it while adding the path remap required for stable diagnostics on
    // Windows.
    if is_coverage_build {
        flags.extend(["-C", "instrument-coverage"].map(str::to_owned));
    }
    flags.push(format!("--remap-path-prefix={package_root}="));

    flags.join(&RUSTFLAGS_SEPARATOR.to_string())
}

#[test]
fn coverage_trybuild_flags_exclude_development_build_flags() {
    let inherited = [
        "-D",
        "warnings",
        "-Zthreads=8",
        "-Zcodegen-backend=cranelift",
        "--cfg=ortho_config_dev_fast",
        "--check-cfg=cfg(ortho_config_dev_fast)",
        "-Clink-arg=-fuse-ld=mold",
    ]
    .join(&RUSTFLAGS_SEPARATOR.to_string());

    let selected = split_encoded_rustflags(&inherited);
    let encoded = compose_trybuild_rustflags(&selected, true, "/workspace/package");
    let arguments = encoded.split(RUSTFLAGS_SEPARATOR).collect::<Vec<_>>();

    assert!(arguments.windows(2).any(|pair| pair == ["-D", "warnings"]));
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["-C", "instrument-coverage"])
    );
    assert!(
        !DEV_FAST_RUSTFLAGS
            .iter()
            .any(|flag| arguments.contains(flag))
    );
}

#[test]
fn empty_encoded_flags_do_not_add_an_empty_argument() {
    let selected = split_encoded_rustflags("");
    let encoded = compose_trybuild_rustflags(&selected, true, "/workspace/package");
    let arguments = encoded.split(RUSTFLAGS_SEPARATOR).collect::<Vec<_>>();

    assert!(arguments.iter().all(|flag| !flag.is_empty()));
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["-C", "instrument-coverage"])
    );
}

#[test]
fn non_coverage_trybuild_flags_preserve_the_development_build_standard() {
    let selected = DEV_FAST_RUSTFLAGS.map(str::to_owned);
    let encoded = compose_trybuild_rustflags(&selected, false, "/workspace/package");
    let arguments = encoded.split(RUSTFLAGS_SEPARATOR).collect::<Vec<_>>();

    assert!(
        DEV_FAST_RUSTFLAGS
            .iter()
            .all(|flag| arguments.contains(flag))
    );
}

#[test]
fn plain_environment_flags_are_selected_when_encoded_flags_are_absent() {
    let inherited = format!("-D warnings {}", DEV_FAST_RUSTFLAGS.join(" "));
    let selected = selected_rustflags(None, Some(&inherited), None, None);
    let encoded = compose_trybuild_rustflags(&selected, false, "/workspace/package");
    let arguments = encoded.split(RUSTFLAGS_SEPARATOR).collect::<Vec<_>>();

    assert!(arguments.windows(2).any(|pair| pair == ["-D", "warnings"]));
    assert!(
        DEV_FAST_RUSTFLAGS
            .iter()
            .all(|flag| arguments.contains(flag))
    );
}

#[test]
fn plain_coverage_flags_keep_warnings_and_remove_development_flags() {
    let inherited = format!(
        "-D warnings -C instrument-coverage {}",
        DEV_FAST_RUSTFLAGS.join(" ")
    );
    let selected = selected_rustflags(None, Some(&inherited), None, None);
    let encoded = compose_trybuild_rustflags(
        &selected,
        has_coverage_instrumentation(&selected),
        "/workspace/package",
    );
    let arguments = encoded.split(RUSTFLAGS_SEPARATOR).collect::<Vec<_>>();

    assert!(arguments.windows(2).any(|pair| pair == ["-D", "warnings"]));
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["-C", "instrument-coverage"])
    );
    assert!(
        !DEV_FAST_RUSTFLAGS
            .iter()
            .any(|flag| arguments.contains(flag))
    );
}

#[test]
fn configuration_flags_are_selected_when_environment_flags_are_absent() {
    let target_flags = if cfg!(target_os = "linux") {
        Some(configured_development_rustflags())
    } else {
        None
    };
    let selected = selected_rustflags(None, None, target_flags.as_deref(), None);
    let configured = configured_development_rustflags();
    let encoded = compose_trybuild_rustflags(&selected, false, "/workspace/package");
    let arguments = encoded.split(RUSTFLAGS_SEPARATOR).collect::<Vec<_>>();

    assert_eq!(selected, configured);
    assert!(
        configured
            .iter()
            .all(|flag| arguments.contains(&flag.as_str()))
    );
}

#[cfg(target_os = "linux")]
#[test]
fn target_configuration_precedes_build_rustflags_environment_override() {
    let target_flags = configured_development_rustflags();
    let selected = selected_rustflags(
        None,
        None,
        Some(target_flags.as_slice()),
        Some("-D warnings"),
    );

    assert_eq!(selected, target_flags);
}

#[test]
fn build_rustflags_is_selected_when_no_target_configuration_matches() {
    let selected = selected_rustflags(None, None, None, Some("-D warnings"));

    assert_eq!(selected, ["-D".to_owned(), "warnings".to_owned()]);
}

#[test]
fn encoded_flags_take_precedence_over_plain_and_configured_flags() {
    let target_rustflags = ["-Zthreads=8".to_owned()];
    let selected = selected_rustflags(
        Some("-D\u{1f}warnings"),
        Some("-C opt-level=0"),
        Some(target_rustflags.as_slice()),
        Some("-C debuginfo=0"),
    );

    assert_eq!(selected, ["-D".to_owned(), "warnings".to_owned()]);
}

#[test]
fn must_use_compile_tests() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os(TRYBUILD_CHILD_ENV).is_none() {
        // Nextest may start workspace tests outside this package. Running the
        // child from the package directory lets trybuild discover the same
        // source root that rustc uses for diagnostic paths.
        let status = Command::new(std::env::current_exe()?)
            .args(["--exact", "must_use_compile_tests", "--nocapture"])
            .env(TRYBUILD_CHILD_ENV, "1")
            .env_remove("CARGO_MANIFEST_DIR")
            .env(
                "CARGO_ENCODED_RUSTFLAGS",
                trybuild_rustflags(env!("CARGO_MANIFEST_DIR")),
            )
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()?;
        if !status.success() {
            return Err(
                std::io::Error::other(format!("trybuild child exited with {status}")).into(),
            );
        }
        return Ok(());
    }

    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/must_use_bridge_ir.rs");
    t.pass("tests/ui/policy_public_api.rs");
    t.compile_fail("tests/ui/must_use_policy_evaluate.rs");
    t.compile_fail("tests/ui/policy_inputs_non_exhaustive.rs");
    Ok(())
}
