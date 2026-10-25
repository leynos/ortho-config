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
    let is_coverage_build = rustflags
        .as_deref()
        .is_some_and(|flags| flags.contains("-C instrument-coverage"));
    compose_trybuild_rustflags(encoded_flags.as_deref(), is_coverage_build, package_root)
}

fn compose_trybuild_rustflags(
    encoded_flags: Option<&str>,
    is_coverage_build: bool,
    package_root: &str,
) -> String {
    let mut flags = encoded_flags
        .unwrap_or_default()
        .split(RUSTFLAGS_SEPARATOR)
        .filter(|flag| !flag.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
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

    let encoded = compose_trybuild_rustflags(Some(&inherited), true, "/workspace/package");
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
    let encoded = compose_trybuild_rustflags(Some(""), true, "/workspace/package");
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
    let inherited = DEV_FAST_RUSTFLAGS.join(&RUSTFLAGS_SEPARATOR.to_string());

    let encoded = compose_trybuild_rustflags(Some(&inherited), false, "/workspace/package");
    let arguments = encoded.split(RUSTFLAGS_SEPARATOR).collect::<Vec<_>>();

    assert!(
        DEV_FAST_RUSTFLAGS
            .iter()
            .all(|flag| arguments.contains(flag))
    );
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
