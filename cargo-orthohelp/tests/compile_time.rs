//! Compile-time UI tests for `cargo-orthohelp`.
//!
//! These tests use `trybuild` to assert that code patterns which should be
//! rejected by the compiler (e.g. ignoring `#[must_use]` return values) do in
//! fact fail to compile.

use std::{ffi::OsString, path::Path, process::Command, time::Duration};

const TRYBUILD_CHILD_ENV: &str = "ORTHOHELP_TRYBUILD_CHILD";
/// Child budget that must expire before nextest's own 960 s tier does.
///
/// The tier is eight 120 s warning periods. A child killed by nextest reports
/// nothing but a timeout, so this budget ends first and lets the parent print
/// the captured streams. The worst finished trybuild binary reached 572.5 s
/// and `must_use_compile_tests` was killed at 600.2 s while still cold, so the
/// budget is set well above both while leaving a minute for reporting.
const TRYBUILD_CHILD_DEADLINE: Duration = Duration::from_secs(900);
const RUSTFLAGS_SEPARATOR: char = '\u{1f}';
const CARGO_CONFIG: &str = include_str!("../../.cargo/config.toml");
const DEV_FAST_RUSTFLAGS: [&str; 5] = [
    "-Zthreads=8",
    "-Zcodegen-backend=cranelift",
    "--cfg=ortho_config_dev_fast",
    "--check-cfg=cfg(ortho_config_dev_fast)",
    "-Clink-arg=-fuse-ld=mold",
];

/// The flags `.cargo/config.toml` selects by default. The Cranelift backend and
/// the `ortho_config_dev_fast` cfg are disabled there (14 November 2026), so
/// they appear only in [`DEV_FAST_RUSTFLAGS`], which coverage still strips.
const DEFAULT_DEV_RUSTFLAGS: [&str; 3] = [
    "-Zthreads=8",
    "--check-cfg=cfg(ortho_config_dev_fast)",
    "-Clink-arg=-fuse-ld=mold",
];

#[derive(Clone, Copy)]
enum CargoConfigSection {
    Build,
    LinuxTarget,
}

impl CargoConfigSection {
    const fn header(self) -> &'static str {
        match self {
            Self::Build => "[build]",
            Self::LinuxTarget => r#"[target.'cfg(target_os = "linux")']"#,
        }
    }
}

#[derive(Clone, Copy)]
struct RustflagsInputs<'a> {
    encoded: Option<&'a str>,
    plain: Option<&'a str>,
    target: Option<&'a [String]>,
    build: Option<&'a str>,
}

/// A compiler-flag variable that exists but cannot be read as text.
#[derive(Debug, thiserror::Error)]
enum RustflagsEnvironmentError {
    #[error("{name} is set but is not valid Unicode; unset it or use a text value")]
    NotUnicode { name: String },
}

/// The raw compiler-flag variables present in the process environment.
///
/// Keeping this separate from flag selection lets the selection rules be
/// tested without touching the environment, and keeps absence distinct from an
/// unreadable value.
struct RustflagsEnvironment {
    encoded: Option<String>,
    plain: Option<String>,
    target: Option<String>,
    build: Option<String>,
}

impl RustflagsEnvironment {
    fn from_process() -> Result<Self, RustflagsEnvironmentError> {
        // Cargo joins triple-specific flags with matching target tables before
        // considering the build-wide value.
        let target_key = format!(
            "CARGO_TARGET_{}_RUSTFLAGS",
            target_tuple::TARGET.to_ascii_uppercase().replace('-', "_")
        );
        Ok(Self {
            encoded: read_flag_variable("CARGO_ENCODED_RUSTFLAGS")?,
            plain: read_flag_variable("RUSTFLAGS")?,
            target: read_flag_variable(&target_key)?,
            build: read_flag_variable("CARGO_BUILD_RUSTFLAGS")?,
        })
    }
}

/// Read one flag variable, reporting an unreadable value instead of dropping
/// it.
///
/// `std::env::var(name).ok()` would fold `NotUnicode` into `NotPresent`, which
/// silently changes the flags the child build receives. A missing variable is
/// genuinely absent, so it stays `None`.
fn read_flag_variable(name: &str) -> Result<Option<String>, RustflagsEnvironmentError> {
    decode_flag_variable(name, std::env::var_os(name))
}

/// Convert one flag variable supplied as an OS string.
fn decode_flag_variable(
    name: &str,
    value: Option<OsString>,
) -> Result<Option<String>, RustflagsEnvironmentError> {
    value
        .map(|text| {
            text.into_string()
                .map_err(|_| RustflagsEnvironmentError::NotUnicode {
                    name: name.to_owned(),
                })
        })
        .transpose()
}

fn trybuild_rustflags(package_root: &Path) -> Result<String, RustflagsEnvironmentError> {
    let environment = RustflagsEnvironment::from_process()?;
    let flags = selected_environment_rustflags(&environment, cfg!(target_os = "linux"));
    let is_coverage_build = has_coverage_instrumentation(&flags);
    Ok(compose_trybuild_rustflags(
        &flags,
        is_coverage_build,
        package_root,
    ))
}

/// Select the child's compiler flags from the variables the process carries.
fn selected_environment_rustflags(
    environment: &RustflagsEnvironment,
    is_linux: bool,
) -> Vec<String> {
    let target_rustflags = matching_target_rustflags(environment.target.as_deref(), is_linux);
    selected_rustflags(RustflagsInputs {
        encoded: environment.encoded.as_deref(),
        plain: environment.plain.as_deref(),
        target: target_rustflags.as_deref(),
        build: environment.build.as_deref(),
    })
}

fn matching_target_rustflags(target: Option<&str>, is_linux: bool) -> Option<Vec<String>> {
    let mut flags = target.map(|value| {
        value
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    });
    if is_linux {
        flags
            .get_or_insert_with(Vec::new)
            .extend(configured_development_rustflags());
    }
    flags
}

fn selected_rustflags(inputs: RustflagsInputs<'_>) -> Vec<String> {
    if let Some(flags) = inputs.encoded {
        return split_encoded_rustflags(flags);
    }
    if let Some(flags) = inputs.plain {
        return flags.split_whitespace().map(str::to_owned).collect();
    }
    // Cargo chooses matching target flags before `[build].rustflags`, even
    // when the latter comes from `CARGO_BUILD_RUSTFLAGS`.
    if let Some(flags) = inputs.target {
        return flags.to_vec();
    }
    if let Some(flags) = inputs.build {
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
    let flag_count = if cfg!(target_os = "linux") { 3 } else { 2 };
    DEFAULT_DEV_RUSTFLAGS
        .iter()
        .take(flag_count)
        .map(|flag| (*flag).to_owned())
        .collect()
}

fn rustflags_in_config_section(section: CargoConfigSection) -> Option<Vec<String>> {
    let section_header = section.header();
    let mut in_section = false;
    let mut section_found = false;
    let mut section_lines = Vec::new();
    for line in CARGO_CONFIG.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if section_found {
                break;
            }
            in_section = trimmed == section_header;
            section_found |= in_section;
        } else if in_section {
            section_lines.push(trimmed);
        }
    }
    if !section_found {
        return None;
    }

    let section_text = section_lines.join("\n");
    let list = section_text
        .split_once("rustflags = [")?
        .1
        .split_once(']')?
        .0;
    list.split(',')
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.strip_prefix('"')?.strip_suffix('"').map(str::to_owned))
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
    package_root: &Path,
) -> String {
    let mut flags = selected_flags.to_vec();
    if is_coverage_build {
        flags.retain(|flag| !DEV_FAST_RUSTFLAGS.contains(&flag.as_str()));
    }
    flags.extend(["--cfg", "trybuild", "--verbose", "--diagnostic-width=140"].map(str::to_owned));

    // Nested trybuild Cargo builds need the requested LLVM instrumentation.
    // Keep it while adding the path remap required for stable diagnostics on
    // Windows.
    if is_coverage_build {
        flags.extend(["-C", "instrument-coverage"].map(str::to_owned));
    }
    let package_root_prefix = package_root.to_string_lossy();
    flags.push(format!("--remap-path-prefix={package_root_prefix}="));

    flags.join(&RUSTFLAGS_SEPARATOR.to_string())
}

#[path = "compile_time/flags_tests.rs"]
mod flags_tests;

#[path = "compile_time/flag_composition_property_tests.rs"]
mod flag_composition_property_tests;

#[test]
fn must_use_compile_tests() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os(TRYBUILD_CHILD_ENV).is_none() {
        // Nextest may start workspace tests outside this package. Running the
        // child from the package directory lets trybuild discover the same
        // source root that rustc uses for diagnostic paths.
        let mut command = Command::new(std::env::current_exe()?);
        command
            .args(["--exact", "must_use_compile_tests", "--nocapture"])
            .env(TRYBUILD_CHILD_ENV, "1")
            .env_remove("CARGO_MANIFEST_DIR")
            .env(
                "CARGO_ENCODED_RUSTFLAGS",
                trybuild_rustflags(Path::new(env!("CARGO_MANIFEST_DIR")))?,
            )
            .current_dir(env!("CARGO_MANIFEST_DIR"));
        let output = test_helpers::process::run_with_deadline(
            command,
            TRYBUILD_CHILD_DEADLINE,
            "trybuild child",
        )?;
        if !output.status.success() {
            return Err(std::io::Error::other(format!(
                "trybuild child exited with {}\n--- stdout ---\n{}--- stderr ---\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            ))
            .into());
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
