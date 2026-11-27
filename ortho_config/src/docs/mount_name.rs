//! Command-name resolution for documentation payloads.
//!
//! The subcommand merge reads its environment and file namespace from
//! `CommandFactory::command().get_name()`, which only `clap::Parser` supplies.
//! A `clap::Args` payload has no command of its own, so its documentation keeps
//! the selector label. Generated documentation cannot add a `CommandFactory`
//! bound to every payload, so it asks through the two `PhantomData`
//! implementations below: method resolution on `&PhantomData` prefers the
//! `CommandFactory` implementation when the bound holds and otherwise
//! dereferences to the label-preserving one. Both implementations are macro
//! support and are hidden from the rendered documentation.

use std::marker::PhantomData;

/// Resolves the clap command name recorded for a documentation payload.
///
/// This trait exists for generated code. Calling it through method syntax on a
/// `&PhantomData<&T>` value selects the `clap::CommandFactory` implementation
/// when `T` implements that trait; otherwise method resolution dereferences to
/// the blanket implementation, which reports no command name.
#[doc(hidden)]
pub trait MountCommandName {
    /// Returns the payload's clap command name, or `None` when it has none.
    fn mount_command_name(&self) -> Option<String>;
}

impl<T> MountCommandName for PhantomData<&T> {
    fn mount_command_name(&self) -> Option<String> {
        None
    }
}

impl<T: clap::CommandFactory> MountCommandName for &PhantomData<&T> {
    fn mount_command_name(&self) -> Option<String> {
        Some(<T as clap::CommandFactory>::command().get_name().to_owned())
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the command-name probe.

    use super::MountCommandName;
    use clap::Parser;
    use std::marker::PhantomData;

    /// A payload with a clap command of its own.
    #[derive(Parser)]
    #[command(name = "payload")]
    struct HasCommand {
        /// Ignored by the probe.
        #[arg(long)]
        value: Option<String>,
    }

    /// A docs-only payload without a command of its own.
    #[derive(clap::Args)]
    struct NoCommand {
        /// Ignored by the probe.
        #[arg(long)]
        value: Option<String>,
    }

    #[test]
    fn a_parser_payload_reports_its_command_name() {
        let probe = PhantomData::<&HasCommand>;
        let name = (&&probe)
            .mount_command_name()
            .expect("a Parser payload has a command");
        assert_eq!(name, "payload", "the declared command name should win");
    }

    #[test]
    fn an_args_payload_reports_no_command_name() {
        let probe = PhantomData::<&NoCommand>;
        assert!(
            probe.mount_command_name().is_none(),
            "an Args payload has no command of its own"
        );
    }
}
