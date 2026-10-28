//! Tests for selecting and composing the trybuild child compiler flags.

use super::*;

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

#[rstest::rstest]
#[case::linux(Some("-D warnings"), true, true)]
#[case::other_target(Some("-D warnings"), false, false)]
fn target_environment_flags_are_kept_with_matching_configuration(
    #[case] target: Option<&str>,
    #[case] is_linux: bool,
    #[case] has_linux_flags: bool,
) {
    let flags = matching_target_rustflags(target, is_linux).expect("target flags must be selected");
    assert!(flags.windows(2).any(|pair| pair == ["-D", "warnings"]));
    assert_eq!(flags.contains(&"-Zthreads=8".to_owned()), has_linux_flags);
    let selected = selected_rustflags(RustflagsInputs {
        encoded: None,
        plain: None,
        target: Some(&flags),
        build: Some("-C debuginfo=0"),
    });
    assert_eq!(selected, flags);
}

#[test]
fn absent_target_flags_without_a_matching_cfg_allow_build_flags() {
    assert!(matching_target_rustflags(None, false).is_none());
}

#[test]
fn target_environment_reaches_encoded_child_flags() {
    const MARKER: &str = "ORTHOHELP_TARGET_FLAGS_PROBE";
    const FLAG: &str = "--cfg=orthohelp_target_flags_probe";
    if std::env::var_os(MARKER).is_some() {
        let flags = trybuild_rustflags(Path::new("/workspace/package"));
        assert!(flags.split(RUSTFLAGS_SEPARATOR).any(|flag| flag == FLAG));
    } else {
        let key = format!(
            "CARGO_TARGET_{}_RUSTFLAGS",
            target_tuple::TARGET.to_ascii_uppercase().replace('-', "_")
        );
        let status = Command::new(std::env::current_exe().expect("find test executable"))
            .args([
                "--exact",
                "flags_tests::target_environment_reaches_encoded_child_flags",
            ])
            .env(MARKER, "1")
            .env(key, FLAG)
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("RUSTFLAGS")
            .status()
            .expect("run target-flags probe");
        assert!(status.success(), "the target-flags probe failed: {status}");
    }
}
