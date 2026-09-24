//! Fixed suffixes used in derived Fluent message identifiers.

/// A fixed final or intermediate identifier segment.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MessageSuffix {
    About,
    LongAbout,
    Usage,
    Version,
    LongVersion,
    AfterHelp,
    AfterLongHelp,
    Args,
    Help,
    LongHelp,
    ValueName,
}

impl AsRef<str> for MessageSuffix {
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
    fn from(value: MessageSuffix) -> Self {
        value.as_ref().to_owned()
    }
}
