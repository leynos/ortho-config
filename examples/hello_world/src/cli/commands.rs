//! Command definitions and validation logic for the hello world example.
//!
//! Keeping these structures in a dedicated module avoids bloating the root CLI
//! module whilst preserving a focused public surface.

use clap::{ArgAction, Parser, ValueEnum};
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

use crate::error::ValidationError;

/// Customisation options for the `greet` command.
#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "HELLO_WORLD")]
pub struct GreetCommand {
    /// Optional preamble printed before the greeting.
    #[arg(long, value_name = "PHRASE", id = "preamble")]
    pub preamble: Option<String>,
    /// Punctuation appended to the greeting when not whispered.
    ///
    /// The `cli_default_as_absent` attribute ensures that file/environment
    /// configuration takes precedence over the clap default when the user
    /// does not explicitly provide `--punctuation` on the command line.
    #[arg(
        long,
        value_name = "TEXT",
        id = "punctuation",
        default_value_t = default_punctuation()
    )]
    #[ortho_config(default = default_punctuation(), cli_default_as_absent)]
    pub punctuation: String,
}

impl Default for GreetCommand {
    fn default() -> Self {
        Self {
            preamble: None,
            punctuation: default_punctuation(),
        }
    }
}

impl GreetCommand {
    /// Ensures user-provided options are well formed.
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] when punctuation or preamble values are
    /// blank after trimming.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.punctuation.trim().is_empty() {
            return Err(ValidationError::BlankPunctuation);
        }
        if self
            .preamble
            .as_deref()
            .is_some_and(|text| text.trim().is_empty())
        {
            return Err(ValidationError::BlankPreamble);
        }
        Ok(())
    }
}

fn default_punctuation() -> String {
    String::from("!")
}

/// Options controlling the `take-leave` workflow.
#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "HELLO_WORLD")]
pub struct TakeLeaveCommand {
    /// Parting phrase to use when saying goodbye.
    #[arg(
        long,
        value_name = "PHRASE",
        id = "parting",
        default_value_t = default_parting()
    )]
    #[ortho_config(default = default_parting())]
    pub parting: String,
    /// Optional preamble printed before the farewell greeting.
    #[arg(long = "preamble", value_name = "PHRASE", id = "farewell_preamble")]
    pub greeting_preamble: Option<String>,
    /// Optional punctuation override appended to the farewell greeting.
    #[arg(long = "punctuation", value_name = "TEXT", id = "farewell_punctuation")]
    pub greeting_punctuation: Option<String>,
    /// Describes how the farewell follow-up will be delivered.
    #[arg(long = "channel", value_enum, id = "channel")]
    pub channel: Option<FarewellChannel>,
    /// Optional reminder delay in minutes.
    #[arg(long = "remind-in", value_name = "MINUTES", id = "remind_in")]
    pub remind_in: Option<u16>,
    /// Optional gift noted in the farewell.
    #[arg(long, value_name = "ITEM", id = "gift")]
    pub gift: Option<String>,
    /// Records whether the caller waves whilst leaving.
    #[arg(long, action = ArgAction::SetTrue, id = "wave")]
    #[ortho_config(default = false)]
    pub wave: bool,
}

impl Default for TakeLeaveCommand {
    fn default() -> Self {
        Self {
            parting: default_parting(),
            greeting_preamble: None,
            greeting_punctuation: None,
            channel: None,
            remind_in: None,
            gift: None,
            wave: false,
        }
    }
}

impl TakeLeaveCommand {
    /// Validates caller-provided farewell customisation.
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] when any validation check fails.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_greeting_overrides()?;
        self.validate_parting()?;
        self.validate_reminder()?;
        self.validate_gift()?;
        Ok(())
    }

    /// Ensures the farewell parting phrase is not blank.
    fn validate_parting(&self) -> Result<(), ValidationError> {
        if self.parting.trim().is_empty() {
            return Err(ValidationError::BlankFarewell);
        }
        Ok(())
    }

    /// Ensures reminder durations fall within the supported range.
    fn validate_reminder(&self) -> Result<(), ValidationError> {
        if self.remind_in.is_some_and(|minutes| minutes == 0) {
            return Err(ValidationError::ReminderOutOfRange);
        }
        Ok(())
    }

    /// Ensures gifts, when provided, are not blank strings.
    fn validate_gift(&self) -> Result<(), ValidationError> {
        if self
            .gift
            .as_deref()
            .is_some_and(|gift| gift.trim().is_empty())
        {
            return Err(ValidationError::BlankGift);
        }
        Ok(())
    }

    fn validate_greeting_overrides(&self) -> Result<(), ValidationError> {
        if self
            .greeting_preamble
            .as_deref()
            .is_some_and(|text| text.trim().is_empty())
        {
            return Err(ValidationError::BlankPreamble);
        }
        if self
            .greeting_punctuation
            .as_deref()
            .is_some_and(|text| text.trim().is_empty())
        {
            return Err(ValidationError::BlankPunctuation);
        }
        Ok(())
    }
}

fn default_parting() -> String {
    String::from("Take care")
}

/// Delivery channels supported by the `take-leave` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum FarewellChannel {
    /// Sends a follow-up via instant message.
    Message,
    /// Schedules a quick voice call.
    Call,
    /// Dispatches a friendly email.
    Email,
}

impl FarewellChannel {
    /// Describes how the farewell will be delivered for user messaging.
    ///
    /// # Examples
    ///
    /// ```
    /// use hello_world::cli::FarewellChannel;
    /// assert_eq!(FarewellChannel::Email.describe(), "an email");
    /// ```
    #[must_use]
    pub const fn describe(&self) -> &'static str {
        match self {
            Self::Message => "a message",
            Self::Call => "a call",
            Self::Email => "an email",
        }
    }
}
