//! Fixed suffixes used in derived Fluent message identifiers.

/// A fixed final or intermediate identifier segment.
///
/// Both identifier producers share this vocabulary: the localization
/// generator builds `ARG_IDS`, and the documentation generator builds the
/// matching IR keys. Sourcing the literals here is what keeps a runtime
/// lookup and its documented identifier from drifting apart.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MessageSuffix {
    /// The command's one-line about text.
    About,
    /// The command's long about text.
    LongAbout,
    /// The command's usage string.
    Usage,
    /// The command's short version.
    Version,
    /// The command's long version.
    LongVersion,
    /// Text shown after the command's help.
    AfterHelp,
    /// Text shown after the command's long help.
    AfterLongHelp,
    /// The intermediate segment grouping an argument's identifiers.
    Args,
    /// An argument's one-line help.
    Help,
    /// An argument's long help.
    LongHelp,
    /// An argument's displayed value name.
    ValueName,
}

impl AsRef<str> for MessageSuffix {
    /// Returns the Fluent segment this suffix contributes.
    fn as_ref(&self) -> &str {
        match self {
            Self::About => "about",
            Self::LongAbout => "long_about",
            Self::Usage => "usage",
            Self::Version => "version",
            Self::LongVersion => "long_version",
            Self::AfterHelp => "after_help",
            Self::AfterLongHelp => "after_long_help",
            Self::Args => "args",
            Self::Help => "help",
            Self::LongHelp => "long_help",
            Self::ValueName => "value_name",
        }
    }
}

impl From<MessageSuffix> for String {
    /// Returns the Fluent segment as an owned string.
    fn from(value: MessageSuffix) -> Self {
        value.as_ref().to_owned()
    }
}
