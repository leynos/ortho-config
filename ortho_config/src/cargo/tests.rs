//! Unit tests for the Cargo external-subcommand wrapper helper.

use super::external_subcommand;
use clap::{Arg, ArgAction, Command, error::ErrorKind};
use rstest::rstest;

fn demo_command() -> Command {
    Command::new("demo").version("1.2.3").arg(
        Arg::new("verbose")
            .long("verbose")
            .action(ArgAction::SetTrue),
    )
}

fn wrapped_demo() -> Command {
    external_subcommand("cargo-demo", "demo", demo_command())
}

fn equivalence_command() -> Command {
    Command::new("demo")
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .action(ArgAction::SetTrue),
        )
        .arg(Arg::new("output").long("output").action(ArgAction::Set))
        .arg(Arg::new("target"))
}

fn required_target_command() -> Command {
    Command::new("demo").arg(Arg::new("target").required(true))
}

fn assert_hedged_bare_invocation_kind(error: &clap::Error) {
    // clap has shifted the exact kind across minor versions; assert
    // membership in the small set of plausible kinds instead.
    assert!(
        matches!(
            error.kind(),
            ErrorKind::UnknownArgument
                | ErrorKind::InvalidSubcommand
                | ErrorKind::MissingSubcommand
        ),
        "unexpected error kind {:?}: {error}",
        error.kind()
    );
}

#[test]
fn wraps_command_under_cargo_parent() {
    let wrapper = wrapped_demo();

    assert_eq!(wrapper.get_name(), "cargo");
    assert_eq!(wrapper.get_bin_name(), Some("cargo"));
    assert!(wrapper.is_subcommand_required_set());
    let subcommands: Vec<&Command> = wrapper.get_subcommands().collect();
    assert_eq!(
        subcommands.len(),
        1,
        "wrapper must nest exactly one subcommand"
    );
    let subcommand = subcommands
        .first()
        .expect("exactly one subcommand was asserted");
    assert_eq!(subcommand.get_name(), "demo");
}

#[test]
fn parses_cargo_dispatch_argv() {
    let matches = wrapped_demo()
        .try_get_matches_from(["cargo-demo", "demo", "--verbose"])
        .expect("Cargo dispatch arguments should parse");

    let demo = matches
        .subcommand_matches("demo")
        .expect("subcommand_required guarantees a subcommand");
    assert!(
        demo.get_flag("verbose"),
        "inner options must be readable one level down"
    );
}

#[test]
fn parses_direct_invocation_argv() {
    let matches = wrapped_demo()
        .try_get_matches_from(["./target/debug/cargo-demo", "demo", "--verbose"])
        .expect("direct invocation with the injected token should parse");

    let demo = matches
        .subcommand_matches("demo")
        .expect("subcommand_required guarantees a subcommand");
    assert!(demo.get_flag("verbose"));
}

#[test]
fn rejects_flag_without_injected_token() {
    let error = wrapped_demo()
        .try_get_matches_from(["cargo-demo", "--verbose"])
        .expect_err("a flag without the injected token should fail");

    assert_hedged_bare_invocation_kind(&error);
}

#[test]
fn rejects_zero_argument_invocation() {
    let error = wrapped_demo()
        .try_get_matches_from(["cargo-demo"])
        .expect_err("bare invocation without arguments should fail");

    assert_eq!(error.kind(), ErrorKind::MissingSubcommand);
}

#[test]
fn renames_inner_command_to_subcommand_name() {
    let wrapper = external_subcommand(
        "cargo-demo",
        "demo",
        Command::new("something-else").arg(
            Arg::new("verbose")
                .long("verbose")
                .action(ArgAction::SetTrue),
        ),
    );

    // `ArgMatches::subcommand_matches` panics on names that were never
    // registered, so the rename is checked on the command itself.
    assert!(wrapper.find_subcommand("demo").is_some());
    assert!(wrapper.find_subcommand("something-else").is_none());

    let matches = wrapper
        .try_get_matches_from(["cargo-demo", "demo", "--verbose"])
        .expect("renamed inner command should parse under the injected name");

    let demo = matches
        .subcommand_matches("demo")
        .expect("demo subcommand should be present");
    assert!(demo.get_flag("verbose"));
}

#[test]
fn sets_inner_display_name_and_resets_bin_name() {
    let wrapper = external_subcommand("cargo-demo", "demo", demo_command().bin_name("stale-bin"));

    let inner = wrapper
        .find_subcommand("demo")
        .expect("wrapper must contain the demo subcommand");
    assert_eq!(inner.get_display_name(), Some("cargo-demo"));
    assert_eq!(
        inner.get_bin_name(),
        None,
        "caller-set bin_name must be reset"
    );
}

#[rstest]
#[case::parse_descent_help(["cargo-demo", "demo", "--help"])]
#[case::help_subcommand(["cargo-demo", "help", "demo"])]
fn caller_bin_name_does_not_leak_into_usage(#[case] argv: [&str; 3]) {
    let wrapper = external_subcommand("cargo-demo", "demo", demo_command().bin_name("stale-bin"));

    let error = wrapper
        .try_get_matches_from(argv)
        .expect_err("help requests short-circuit parsing");

    let rendered = error.to_string();
    assert!(
        rendered.contains("Usage: cargo demo [OPTIONS]"),
        "unexpected usage rendering:\n{rendered}"
    );
}

#[test]
fn version_renders_installed_binary_name() {
    let error = wrapped_demo()
        .try_get_matches_from(["cargo-demo", "demo", "--version"])
        .expect_err("version requests short-circuit parsing");

    assert_eq!(error.kind(), ErrorKind::DisplayVersion);
    let rendered = error.to_string();
    assert!(
        rendered.starts_with("cargo-demo "),
        "version output should name the installed binary, got: {rendered:?}"
    );
    assert!(
        rendered.contains("1.2.3"),
        "inner version must be preserved, got: {rendered:?}"
    );
}

#[test]
fn top_level_version_is_rejected() {
    let error = wrapped_demo()
        .try_get_matches_from(["cargo-demo", "--version"])
        .expect_err("the synthetic parent carries no version");

    assert_hedged_bare_invocation_kind(&error);
}

#[test]
fn supports_nested_inner_subcommands() {
    let wrapper = external_subcommand(
        "cargo-demo",
        "demo",
        Command::new("demo").subcommand(Command::new("build")),
    );

    let matches = wrapper
        .try_get_matches_from(["cargo-demo", "demo", "build"])
        .expect("nested inner subcommands should parse");
    let demo = matches
        .subcommand_matches("demo")
        .expect("demo subcommand should be present");
    assert!(demo.subcommand_matches("build").is_some());

    let help_wrapper = external_subcommand(
        "cargo-demo",
        "demo",
        Command::new("demo").subcommand(Command::new("build")),
    );
    let error = help_wrapper
        .try_get_matches_from(["cargo-demo", "demo", "build", "--help"])
        .expect_err("help requests short-circuit parsing");
    let rendered = error.to_string();
    assert!(
        rendered.contains("Usage: cargo demo build"),
        "nested usage should render the full dispatch chain:\n{rendered}"
    );
}

#[test]
fn preserves_required_inner_arguments() {
    let missing = external_subcommand("cargo-demo", "demo", required_target_command())
        .try_get_matches_from(["cargo-demo", "demo"])
        .expect_err("missing required inner argument should fail");
    assert_eq!(missing.kind(), ErrorKind::MissingRequiredArgument);

    let matches = external_subcommand("cargo-demo", "demo", required_target_command())
        .try_get_matches_from(["cargo-demo", "demo", "release"])
        .expect("present required inner argument should parse");
    let demo = matches
        .subcommand_matches("demo")
        .expect("demo subcommand should be present");
    assert_eq!(
        demo.get_one::<String>("target").map(String::as_str),
        Some("release")
    );
}

#[rstest]
#[case::no_options(&[])]
#[case::flag_present(&["--verbose"])]
#[case::option_with_value(&["--output", "build/log.txt"])]
#[case::positional_value(&["release"])]
#[case::combined(&["release", "--verbose", "--output", "out.txt"])]
fn wrapped_parse_matches_unwrapped_parse(#[case] tail: &[&str]) {
    let wrapped_argv: Vec<&str> = ["cargo-demo", "demo"]
        .iter()
        .copied()
        .chain(tail.iter().copied())
        .collect();
    let wrapped = external_subcommand("cargo-demo", "demo", equivalence_command())
        .try_get_matches_from(wrapped_argv)
        .expect("wrapped parse should succeed");
    let demo = wrapped
        .subcommand_matches("demo")
        .expect("subcommand_required guarantees a subcommand");

    let unwrapped_argv: Vec<&str> = std::iter::once("demo")
        .chain(tail.iter().copied())
        .collect();
    let unwrapped = equivalence_command()
        .try_get_matches_from(unwrapped_argv)
        .expect("unwrapped parse should succeed");

    assert_eq!(demo.get_flag("verbose"), unwrapped.get_flag("verbose"));
    assert_eq!(
        demo.get_one::<String>("output").map(String::as_str),
        unwrapped.get_one::<String>("output").map(String::as_str)
    );
    assert_eq!(
        demo.get_one::<String>("target").map(String::as_str),
        unwrapped.get_one::<String>("target").map(String::as_str)
    );
}

#[test]
fn transposed_names_trip_debug_assertion() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        external_subcommand("demo", "cargo-demo", Command::new("demo"))
    }));
    assert!(
        result.is_err(),
        "transposed names must trip the debug assertion"
    );
}

#[test]
fn empty_subcommand_name_trips_debug_assertion() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        external_subcommand("cargo-", "", Command::new("demo"))
    }));
    assert!(
        result.is_err(),
        "an empty subcommand name must trip the debug assertion"
    );
}

#[test]
fn reserved_help_name_trips_debug_assertion() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        external_subcommand("cargo-help", "help", Command::new("demo"))
    }));
    assert!(
        result.is_err(),
        "the reserved 'help' name must trip the debug assertion"
    );
}
