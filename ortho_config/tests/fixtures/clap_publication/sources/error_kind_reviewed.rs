//! Synthetic stand-in for the reviewed `clap_builder` inventory.
//!
//! It mirrors the supported identity's 17 variants so the gate's counting code
//! can be exercised without depending on a registry cache layout.

/// Kinds of error produced while parsing command-line arguments.
#[non_exhaustive]
pub enum ErrorKind {
    /// A value was supplied that failed validation.
    InvalidValue,
    /// An argument was supplied that the command does not define.
    UnknownArgument,
    /// A subcommand was supplied that the command does not define.
    InvalidSubcommand,
    /// An argument was written without the required equals sign.
    NoEquals,
    /// A value parser rejected the supplied value.
    ValueValidation,
    /// An argument received more values than it accepts.
    TooManyValues,
    /// An argument received fewer values than it requires.
    TooFewValues,
    /// An argument received the wrong number of values.
    WrongNumberOfValues,
    /// Two arguments cannot be used together.
    ArgumentConflict,
    /// A required argument is absent.
    MissingRequiredArgument,
    /// A required subcommand is absent.
    MissingSubcommand,
    /// An argument value was not valid UTF-8.
    InvalidUtf8,
    /// An input or output operation failed.
    Io,
    /// The generated message could not be formatted.
    Format,
    /// The invocation requested help.
    DisplayHelp,
    /// The invocation requested help because input was missing.
    DisplayHelpOnMissingArgumentOrSubcommand,
    /// The invocation requested the version.
    DisplayVersion,
}