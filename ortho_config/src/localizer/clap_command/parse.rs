//! Localization-aware parsing for `clap` command trees.
//!
//! This module connects three pieces that are otherwise easy to wire
//! inconsistently: [`clap::Parser`] implementations, [`LocalizeCmd`] command
//! metadata rewriting, and [`crate::Localizer`] error-message lookup. It keeps
//! the parse path responsible for both building a localized command tree before
//! `clap` sees the arguments and localizing any parse or `from_arg_matches`
//! error after `clap` rejects them.
//!
//! Use [`parse_localized_command`] when the caller already has a command tree
//! or needs an explicit catalogue key root via [`LocalizeCmd::with_base`]. Use
//! [`LocalizedParse`] for the common zero-configuration path where the default
//! catalogue root should come from the parser's binary name or command name.

use super::LocalizeCmd;
use crate::{Localizer, localize_clap_error_with_command};
use clap::{ArgMatches, Command, FromArgMatches, Parser};
use std::ffi::OsString;

/// Parses arguments with a pre-built localized command and localizes parse
/// errors through the supplied localizer.
///
/// This is the base-agnostic primitive for applications that need to override
/// the command identifier base before parsing.
///
/// # Errors
///
/// Returns `clap::Error` when clap rejects the input or when the parsed
/// [`ArgMatches`] cannot be converted into `P`.
///
/// # Panics
///
/// Panics when the provided command contains identifiers that cannot be
/// represented as Fluent message identifiers. This matches the
/// [`LocalizeCmd::localize`] and [`crate::message_id_for`] contract.
///
/// # Examples
///
/// ```rust
/// use clap::{CommandFactory, Parser};
/// use ortho_config::{
///     LocalizeCmd, NoOpLocalizer, parse_localized_command,
/// };
///
/// #[derive(Debug, Parser)]
/// #[command(name = "demo", bin_name = "demo")]
/// struct Cli {
///     #[arg(long)]
///     verbose: bool,
/// }
///
/// let localizer = NoOpLocalizer::new();
/// let command = Cli::command().with_base("acme.demo").localize(&localizer);
/// let (cli, matches) =
///     parse_localized_command::<Cli, _, _>(command, ["demo", "--verbose"], &localizer)?;
///
/// assert!(cli.verbose);
/// assert!(matches.get_flag("verbose"));
/// # Ok::<(), clap::Error>(())
/// ```
pub fn parse_localized_command<P, I, T>(
    localized_command: Command,
    iter: I,
    localizer: &dyn Localizer,
) -> Result<(P, ArgMatches), clap::Error>
where
    P: FromArgMatches,
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let mut command = localized_command;
    let matches = command
        .try_get_matches_from_mut(iter)
        .map_err(|err| localize_clap_error_with_command(err, localizer, Some(&command)))?;
    let value = P::from_arg_matches(&matches).map_err(|err| {
        localize_clap_error_with_command(err.with_cmd(&command), localizer, Some(&command))
    })?;

    Ok((value, matches))
}

/// Blanket extension trait for parsing any `clap::Parser` type with localized
/// command metadata and errors.
///
/// The default implementation derives the identifier base from the command's
/// `bin_name`, falling back to the command name. Use
/// [`parse_localized_command`] with [`LocalizeCmd::with_base`] when catalogue
/// keys need a different root.
///
/// # Panics
///
/// The methods panic when the command contains identifiers that cannot be
/// represented as Fluent message identifiers. This matches the
/// [`LocalizeCmd::localize`] and [`crate::message_id_for`] contract.
///
/// # Examples
///
/// ```rust
/// use clap::Parser;
/// use ortho_config::{LocalizedParse, NoOpLocalizer};
///
/// #[derive(Debug, Parser)]
/// #[command(name = "demo", bin_name = "demo")]
/// struct Cli {
///     #[arg(long)]
///     verbose: bool,
/// }
///
/// let localizer = NoOpLocalizer::new();
/// let cli = Cli::try_parse_localized_from(["demo", "--verbose"], &localizer)?;
///
/// assert!(cli.verbose);
/// # Ok::<(), clap::Error>(())
/// ```
pub trait LocalizedParse: Parser {
    /// Parses arguments from the process environment with localized command
    /// metadata and errors.
    ///
    /// # Errors
    ///
    /// Returns `clap::Error` when clap rejects the process arguments or when
    /// the resulting matches cannot be converted into `Self`.
    ///
    /// # Panics
    ///
    /// Panics when the command contains identifiers that cannot be represented
    /// as Fluent message identifiers.
    fn try_parse_localized(localizer: &dyn Localizer) -> Result<Self, clap::Error> {
        Self::try_parse_localized_from(std::env::args_os(), localizer)
    }

    /// Parses the supplied arguments with localized command metadata and
    /// errors.
    ///
    /// # Errors
    ///
    /// Returns `clap::Error` when clap rejects the supplied arguments or when
    /// the resulting matches cannot be converted into `Self`.
    ///
    /// # Panics
    ///
    /// Panics when the command contains identifiers that cannot be represented
    /// as Fluent message identifiers.
    fn try_parse_localized_from<I, T>(
        iter: I,
        localizer: &dyn Localizer,
    ) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        Self::try_parse_localized_with_matches(iter, localizer).map(|(value, _matches)| value)
    }

    /// Parses the supplied arguments, returning both the typed parser and raw
    /// `clap` matches.
    ///
    /// # Errors
    ///
    /// Returns `clap::Error` when clap rejects the supplied arguments or when
    /// the resulting matches cannot be converted into `Self`.
    ///
    /// # Panics
    ///
    /// Panics when the command contains identifiers that cannot be represented
    /// as Fluent message identifiers.
    fn try_parse_localized_with_matches<I, T>(
        iter: I,
        localizer: &dyn Localizer,
    ) -> Result<(Self, ArgMatches), clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        parse_localized_command(Self::command().localize(localizer), iter, localizer)
    }
}

impl<P: Parser> LocalizedParse for P {}
