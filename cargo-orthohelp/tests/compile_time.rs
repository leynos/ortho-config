//! Compile-time UI tests for `cargo-orthohelp`.
//!
//! These tests use `trybuild` to assert that code patterns which should be
//! rejected by the compiler (e.g. ignoring `#[must_use]` return values) do in
//! fact fail to compile.

use std::{path::Path, process::Command};

const TRYBUILD_CHILD_ENV: &str = "ORTHOHELP_TRYBUILD_CHILD";
const RUSTFLAGS_SEPARATOR: char = '\u{1f}';
const CARGO_CONFIG: &str = include_str!("../../.cargo/config.toml");
const DEV_FAST_RUSTFLAGS: [&str; 5] = [
    "-Zthreads=8",
    "-Zcodegen-backend=cranelift",
    "--cfg=ortho_config_dev_fast",
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

fn trybuild_rustflags(package_root: &Path) -> String {
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
    let flags = selected_rustflags(RustflagsInputs {
        encoded: encoded_flags.as_deref(),
        plain: rustflags.as_deref(),
        target: target_rustflags.as_deref(),
        build: build_rustflags.as_deref(),
    });
    let is_coverage_build = has_coverage_instrumentation(&flags);
    compose_trybuild_rustflags(&flags, is_coverage_build, package_root)
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
    let flag_count = if cfg!(target_os = "linux") { 5 } else { 4 };
    DEV_FAST_RUSTFLAGS
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
        .filter(|line| !line.is_empty())
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
    let encoded = compose_trybuild_rustflags(&selected, true, Path::new("/workspace/package"));
    let arguments = encoded.split(RUSTFLAGS_SEPARATOR).collect::<Vec<_>>();

    assert!(arguments.windows(2).any(|pair| pair == ["-D", "warnings"]));
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["-C", "instrument-coverage"])
    );
    assert!(!arguments.windows(2).any(|pair| pair == ["-A", "dead_code"]));
    assert!(
        !DEV_FAST_RUSTFLAGS
            .iter()
            .any(|flag| arguments.contains(flag))
    );
}

#[test]
fn empty_encoded_flags_do_not_add_an_empty_argument() {
    let selected = split_encoded_rustflags("");
    let encoded = compose_trybuild_rustflags(&selected, true, Path::new("/workspace/package"));
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
    let encoded = compose_trybuild_rustflags(&selected, false, Path::new("/workspace/package"));
    let arguments = encoded.split(RUSTFLAGS_SEPARATOR).collect::<Vec<_>>();

    assert!(
        DEV_FAST_RUSTFLAGS
            .iter()
            .all(|flag| arguments.contains(flag))
    );
    assert!(!arguments.windows(2).any(|pair| pair == ["-A", "dead_code"]));
}

#[test]
fn plain_environment_flags_are_selected_when_encoded_flags_are_absent() {
    let inherited = format!("-D warnings {}", DEV_FAST_RUSTFLAGS.join(" "));
    let selected = selected_rustflags(RustflagsInputs {
        encoded: None,
        plain: Some(&inherited),
        target: None,
        build: None,
    });
    let encoded = compose_trybuild_rustflags(&selected, false, Path::new("/workspace/package"));
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
    let selected = selected_rustflags(RustflagsInputs {
        encoded: None,
        plain: Some(&inherited),
        target: None,
        build: None,
    });
    let encoded = compose_trybuild_rustflags(
        &selected,
        has_coverage_instrumentation(&selected),
        Path::new("/workspace/package"),
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
    let selected = selected_rustflags(RustflagsInputs {
        encoded: None,
        plain: None,
        target: target_flags.as_deref(),
        build: None,
    });
    let configured = configured_development_rustflags();
    let encoded = compose_trybuild_rustflags(&selected, false, Path::new("/workspace/package"));
    let arguments = encoded.split(RUSTFLAGS_SEPARATOR).collect::<Vec<_>>();

    assert_eq!(selected, configured);
    assert!(
        configured
            .iter()
            .all(|flag| arguments.contains(&flag.as_str()))
    );
}

#[test]
fn development_build_flags_match_the_cargo_configuration() {
    let build_flags = rustflags_in_config_section(CargoConfigSection::Build)
        .expect("Cargo config should define [build].rustflags");
    let linux_flags = rustflags_in_config_section(CargoConfigSection::LinuxTarget)
        .expect("Cargo config should define Linux target rustflags");

    let expected_build_flags = DEV_FAST_RUSTFLAGS[..4]
        .iter()
        .map(|flag| (*flag).to_owned())
        .collect::<Vec<_>>();
    let expected_linux_flags = DEV_FAST_RUSTFLAGS
        .iter()
        .map(|flag| (*flag).to_owned())
        .collect::<Vec<_>>();

    assert_eq!(build_flags, expected_build_flags);
    assert_eq!(linux_flags, expected_linux_flags);
    assert!(!build_flags.contains(&DEV_FAST_RUSTFLAGS[4].to_owned()));
}

#[cfg(target_os = "linux")]
#[test]
fn target_configuration_precedes_build_rustflags_environment_override() {
    let target_flags = configured_development_rustflags();
    let selected = selected_rustflags(RustflagsInputs {
        encoded: None,
        plain: None,
        target: Some(target_flags.as_slice()),
        build: Some("-D warnings"),
    });

    assert_eq!(selected, target_flags);
}

#[test]
fn build_rustflags_is_selected_when_no_target_configuration_matches() {
    let selected = selected_rustflags(RustflagsInputs {
        encoded: None,
        plain: None,
        target: None,
        build: Some("-D warnings"),
    });

    assert_eq!(selected, ["-D".to_owned(), "warnings".to_owned()]);
}

#[test]
fn encoded_flags_take_precedence_over_plain_and_configured_flags() {
    let target_rustflags = ["-Zthreads=8".to_owned()];
    let selected = selected_rustflags(RustflagsInputs {
        encoded: Some("-D\u{1f}warnings"),
        plain: Some("-C opt-level=0"),
        target: Some(target_rustflags.as_slice()),
        build: Some("-C debuginfo=0"),
    });

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
                trybuild_rustflags(Path::new(env!("CARGO_MANIFEST_DIR"))),
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
