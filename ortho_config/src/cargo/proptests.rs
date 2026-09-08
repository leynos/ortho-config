//! Property coverage for Cargo wrapper argument preservation.

use super::external_subcommand;
use clap::{Arg, ArgAction, ArgMatches, Command};
use proptest::{option, prelude::*, test_runner::TestCaseError};

#[derive(Debug)]
struct CommandShape {
    supports_verbose: bool,
    supports_output: bool,
    supports_target: bool,
}

#[derive(Debug)]
struct InvocationCase {
    shape: CommandShape,
    passes_verbose: bool,
    output: Option<String>,
    target: Option<String>,
    places_target_first: bool,
}

impl InvocationCase {
    fn command(&self) -> Command {
        let mut command = Command::new("demo");
        if self.shape.supports_verbose {
            command = command.arg(
                Arg::new("verbose")
                    .long("verbose")
                    .action(ArgAction::SetTrue),
            );
        }
        if self.shape.supports_output {
            command = command.arg(Arg::new("output").long("output").action(ArgAction::Set));
        }
        if self.shape.supports_target {
            command = command.arg(Arg::new("target"));
        }
        command
    }

    fn tail(&self) -> Vec<String> {
        let mut tail = Vec::new();
        if self.places_target_first {
            self.push_target(&mut tail);
        }
        if self.passes_verbose {
            tail.push("--verbose".to_owned());
        }
        if let Some(output) = &self.output {
            tail.extend(["--output".to_owned(), output.clone()]);
        }
        if !self.places_target_first {
            self.push_target(&mut tail);
        }
        tail
    }

    fn push_target(&self, tail: &mut Vec<String>) {
        if let Some(target) = &self.target {
            tail.push(target.clone());
        }
    }
}

fn invocation_case() -> impl Strategy<Value = InvocationCase> {
    (
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        option::of("[a-z][a-z0-9]{0,11}"),
        any::<bool>(),
        option::of("[a-z][a-z0-9]{0,11}"),
        any::<bool>(),
    )
        .prop_map(
            |(
                supports_verbose,
                passes_verbose,
                supports_output,
                output,
                supports_target,
                target,
                places_target_first,
            )| InvocationCase {
                shape: CommandShape {
                    supports_verbose,
                    supports_output,
                    supports_target,
                },
                passes_verbose: supports_verbose && passes_verbose,
                output: supports_output.then_some(output).flatten(),
                target: supports_target.then_some(target).flatten(),
                places_target_first,
            },
        )
}

fn parse_wrapper(case: &InvocationCase, tail: &[String]) -> Result<ArgMatches, TestCaseError> {
    let argv = std::iter::once("cargo-demo".to_owned())
        .chain(std::iter::once("demo".to_owned()))
        .chain(tail.iter().cloned());
    external_subcommand("cargo-demo", "demo", case.command())
        .try_get_matches_from(argv)
        .map_err(|error| TestCaseError::fail(format!("wrapped parse failed: {error}")))
}

fn parse_unwrapped(case: &InvocationCase, tail: &[String]) -> Result<ArgMatches, TestCaseError> {
    let argv = std::iter::once("demo".to_owned()).chain(tail.iter().cloned());
    case.command()
        .try_get_matches_from(argv)
        .map_err(|error| TestCaseError::fail(format!("unwrapped parse failed: {error}")))
}

fn demo_matches(matches: &ArgMatches) -> Result<&ArgMatches, TestCaseError> {
    matches
        .subcommand_matches("demo")
        .ok_or_else(|| TestCaseError::fail("wrapped parse omitted the demo subcommand"))
}

proptest! {
    /// The wrapper preserves every configured leaf argument for correlated,
    /// valid tails across supported hand-built command shapes.
    #[test]
    fn wrapper_preserves_generated_leaf_command_matches(case in invocation_case()) {
        let tail = case.tail();
        let wrapped_matches = parse_wrapper(&case, &tail)?;
        let wrapped = demo_matches(&wrapped_matches)?;
        let unwrapped = parse_unwrapped(&case, &tail)?;

        if case.shape.supports_verbose {
            prop_assert_eq!(wrapped.get_flag("verbose"), unwrapped.get_flag("verbose"));
        }
        if case.shape.supports_output {
            prop_assert_eq!(
                wrapped.get_one::<String>("output"),
                unwrapped.get_one::<String>("output")
            );
        }
        if case.shape.supports_target {
            prop_assert_eq!(
                wrapped.get_one::<String>("target"),
                unwrapped.get_one::<String>("target")
            );
        }
    }
}
