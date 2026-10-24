//! Compile-time UI tests for `cargo-orthohelp`.
//!
//! These tests use `trybuild` to assert that code patterns which should be
//! rejected by the compiler (e.g. ignoring `#[must_use]` return values) do in
//! fact fail to compile.

use std::process::Command;

const TRYBUILD_CHILD_ENV: &str = "ORTHOHELP_TRYBUILD_CHILD";
const RUSTFLAGS_SEPARATOR: char = '\u{1f}';

fn trybuild_rustflags(package_root: &str) -> String {
    let mut flags = std::env::var_os("CARGO_ENCODED_RUSTFLAGS")
        .map(|encoded| {
            encoded
                .to_string_lossy()
                .split(RUSTFLAGS_SEPARATOR)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
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

    // trybuild preserves coverage instrumentation when it replaces RUSTFLAGS
    // for its nested Cargo builds. Keep that behaviour while adding the path
    // remap required for stable diagnostics on Windows.
    if std::env::var("RUSTFLAGS")
        .is_ok_and(|rustflags| rustflags.contains("-C instrument-coverage"))
    {
        flags.extend(["-C", "instrument-coverage"].map(str::to_owned));
    }
    flags.push(format!("--remap-path-prefix={package_root}="));

    flags.join(&RUSTFLAGS_SEPARATOR.to_string())
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
