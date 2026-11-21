//! Drifted inventory: two fewer variants than the reviewed total.
//!
//! The removal of `InvalidUtf8` and `Format` stands in for an upstream release
//! the matrix has not been reviewed against.

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
    /// An input or output operation failed.
    Io,
    /// The invocation requested help.
    DisplayHelp,
    /// The invocation requested help because input was missing.
    DisplayHelpOnMissingArgumentOrSubcommand,
    /// The invocation requested the version.
    DisplayVersion,
}