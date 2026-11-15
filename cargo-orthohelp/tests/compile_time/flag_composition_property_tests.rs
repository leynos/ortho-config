//! Property tests for composing the trybuild child's compiler flags.
//!
//! The composition step rewrites whatever flag list the child inherits, so the
//! examples in `flags_tests` only pin the cases we thought to write down.
//! These properties state the rewrite for any separator-free list, including
//! duplicates, development flags, and flags in any order.

use super::*;
use proptest::prelude::*;

/// Flags that occur in inherited lists, including every development flag that
/// coverage composition removes.
fn known_flag() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("-D".to_owned()),
        Just("warnings".to_owned()),
        Just("-A".to_owned()),
        Just("dead_code".to_owned()),
        Just("-C".to_owned()),
        Just("debuginfo=0".to_owned()),
        Just("-Cinstrument-coverage".to_owned()),
        Just("-Zthreads=8".to_owned()),
        Just("-Zcodegen-backend=cranelift".to_owned()),
        Just("--cfg=ortho_config_dev_fast".to_owned()),
        Just("--check-cfg=cfg(ortho_config_dev_fast)".to_owned()),
        Just("-Clink-arg=-fuse-ld=mold".to_owned()),
        // An unfamiliar flag must survive composition untouched.
        "[a-z][a-z0-9=+/_.-]{0,15}",
    ]
}

/// A bounded, possibly duplicated and arbitrary ordered inherited flag list.
fn inherited_flags() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(known_flag(), 0..12)
}

/// Split the composer's encoded output back into individual flags.
fn split_composed(encoded: &str) -> Vec<String> {
    encoded
        .split(RUSTFLAGS_SEPARATOR)
        .map(str::to_owned)
        .collect()
}

/// The flags the composer appends after the inherited list.
fn child_trailer(is_coverage_build: bool, package_root: &Path) -> Vec<String> {
    let mut trailer = ["--cfg", "trybuild", "--verbose", "--diagnostic-width=140"]
        .map(str::to_owned)
        .to_vec();
    if is_coverage_build {
        trailer.extend(["-C", "instrument-coverage"].map(str::to_owned));
    }
    trailer.push(format!(
        "--remap-path-prefix={}=",
        package_root.to_string_lossy()
    ));
    trailer
}

/// Count the `-C instrument-coverage` pairs in a flag list.
fn count_instrumentation(flags: &[String]) -> usize {
    flags
        .windows(2)
        .filter(|pair| {
            matches!(
                pair,
                [command, value] if command == "-C" && value == "instrument-coverage"
            )
        })
        .count()
}

/// The inherited flags that composition must keep, in their original order.
fn retained_flags(flags: &[String], is_coverage_build: bool) -> Vec<String> {
    flags
        .iter()
        .filter(|flag| !is_coverage_build || !DEV_FAST_RUSTFLAGS.contains(&flag.as_str()))
        .cloned()
        .collect()
}

proptest! {
    #[test]
    fn coverage_composition_removes_only_development_flags(flags in inherited_flags()) {
        let package_root = Path::new("/workspace/package");
        let composed = compose_trybuild_rustflags(&flags, true, package_root);
        let arguments = split_composed(&composed);

        // The encoding is only unambiguous while no flag embeds the separator.
        prop_assert!(flags.iter().all(|flag| !flag.contains(RUSTFLAGS_SEPARATOR)));
        prop_assert!(arguments.iter().all(|flag| !flag.is_empty()));

        let retained = retained_flags(&flags, true);
        let trailer = child_trailer(true, package_root);
        prop_assert_eq!(arguments.len(), retained.len() + trailer.len());
        let (head, tail) = arguments.split_at(retained.len());
        prop_assert_eq!(head, retained.as_slice());
        prop_assert_eq!(tail, trailer.as_slice());

        // Coverage composition keeps any inherited pair and adds one of its
        // own, so the child always requests instrumentation exactly once more.
        prop_assert_eq!(
            count_instrumentation(&arguments),
            count_instrumentation(&flags) + 1
        );
        prop_assert!(
            !DEV_FAST_RUSTFLAGS
                .iter()
                .any(|flag| arguments.iter().any(|argument| argument == flag))
        );
    }

    #[test]
    fn non_coverage_composition_preserves_the_input(flags in inherited_flags()) {
        let package_root = Path::new("/workspace/package");
        let composed = compose_trybuild_rustflags(&flags, false, package_root);
        let arguments = split_composed(&composed);

        prop_assert!(flags.iter().all(|flag| !flag.contains(RUSTFLAGS_SEPARATOR)));
        prop_assert!(arguments.iter().all(|flag| !flag.is_empty()));

        let trailer = child_trailer(false, package_root);
        prop_assert_eq!(arguments.len(), flags.len() + trailer.len());
        // The inherited list is a prefix, untouched and in order.
        let (head, tail) = arguments.split_at(flags.len());
        prop_assert_eq!(head, flags.as_slice());
        prop_assert_eq!(tail, trailer.as_slice());

        // Nothing is appended, so inherited instrumentation is unchanged.
        prop_assert_eq!(count_instrumentation(&arguments), count_instrumentation(&flags));
    }
}
