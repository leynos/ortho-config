//! Greeting planning and rendering for the `hello_world` example.
use crate::cli::{DeliveryMode, GreetCommand, HelloWorldCli, TakeLeaveCommand};
use crate::error::HelloWorldError;
use std::io::{self, Write};

/// Computed greeting ready for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreetingPlan {
    /// Fully formatted greeting after mode-specific punctuation and casing.
    message: String,
    /// Validated delivery mode retained for test inspection.
    mode: DeliveryMode,
    /// Whitespace-trimmed preamble retained when non-empty.
    preamble: Option<String>,
}

impl GreetingPlan {
    /// Returns the formatted greeting message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the delivery mode associated with the greeting.
    #[cfg(test)]
    #[must_use]
    pub const fn mode(&self) -> DeliveryMode {
        self.mode
    }

    /// Returns the optional preamble preceding the greeting.
    #[must_use]
    pub fn preamble(&self) -> Option<&str> {
        self.preamble.as_deref()
    }
}

/// Computed farewell including the greeting sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeLeavePlan {
    /// Greeting produced from the farewell command's greeting overrides.
    greeting: GreetingPlan,
    /// Formatted farewell sentence, including any requested follow-up details.
    farewell: String,
}

impl TakeLeavePlan {
    /// Returns the embedded greeting.
    #[must_use]
    pub const fn greeting(&self) -> &GreetingPlan {
        &self.greeting
    }

    /// Returns the farewell description.
    #[must_use]
    pub fn farewell(&self) -> &str {
        &self.farewell
    }
}

/// Builds a [`GreetingPlan`] from the resolved configuration and command options.
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when validation fails or greeting defaults
/// cannot be loaded.
///
/// # Examples
///
/// ```rust
/// use hello_world::cli::{GreetCommand, HelloWorldCli};
/// use hello_world::error::HelloWorldError;
/// use hello_world::message::build_plan;
///
/// fn demo() -> Result<(), HelloWorldError> {
///     let mut config = HelloWorldCli::default();
///     config.recipient = String::from("Ada Lovelace");
///     config.salutations = vec![String::from("Hello")];
///
///     let command = GreetCommand::default();
///     let plan = build_plan(&config, &command)?;
///
///     assert_eq!(plan.message(), "Hello, Ada Lovelace!");
///     assert_eq!(plan.preamble(), None);
///     Ok(())
/// }
///
/// assert!(demo().is_ok());
/// ```
pub fn build_plan(
    config: &HelloWorldCli,
    command: &GreetCommand,
) -> Result<GreetingPlan, HelloWorldError> {
    config.validate()?;
    command.validate()?;
    let mode = config.delivery_mode();
    let salutation = config.trimmed_salutations().join(" ");
    let recipient = &config.recipient;
    let base = format!("{salutation}, {recipient}");
    let trimmed_preamble = command
        .preamble
        .as_ref()
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
        .map(String::from);
    let punctuation = command.punctuation.trim();
    let message = match mode {
        DeliveryMode::Standard => format!("{base}{punctuation}"),
        DeliveryMode::Enthusiastic => {
            let shout = base.to_uppercase();
            format!("{shout}{punctuation}")
        }
        DeliveryMode::Quiet => format!("{base}..."),
    };
    Ok(GreetingPlan {
        message,
        mode,
        preamble: trimmed_preamble,
    })
}

/// Builds a [`TakeLeavePlan`] describing the farewell workflow.
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when building the greeting plan fails or the
/// farewell configuration is invalid.
///
/// # Examples
///
/// ```rust
/// use hello_world::cli::{HelloWorldCli, TakeLeaveCommand};
/// use hello_world::error::HelloWorldError;
/// use hello_world::message::build_take_leave_plan;
///
/// fn demo() -> Result<(), HelloWorldError> {
///     let mut config = HelloWorldCli::default();
///    config.recipient = String::from("Ada Lovelace");
///    config.salutations = vec![String::from("Hello there")];
///
///    let mut command = TakeLeaveCommand::default();
///    command.parting = String::from("Cheerio");
///    command.greeting_preamble = Some(String::from("Before we go"));
///    command.greeting_punctuation = Some(String::from("?"));
///
///    let plan = build_take_leave_plan(&config, &command)?;
///
///    assert_eq!(plan.greeting().preamble(), Some("Before we go"));
///    assert_eq!(
///        plan.greeting().message(),
///        "Hello there, Ada Lovelace?"
///    );
///    assert_eq!(plan.farewell(), "Cheerio, Ada Lovelace.");
///    Ok(())
/// }
///
/// assert!(demo().is_ok());
/// ```
pub fn build_take_leave_plan(
    config: &HelloWorldCli,
    command: &TakeLeaveCommand,
) -> Result<TakeLeavePlan, HelloWorldError> {
    config.validate()?;
    command.validate()?;

    let greeting_defaults = build_greeting_defaults(command)?;
    let greeting = build_plan(config, &greeting_defaults)?;
    let farewell = format_farewell_message(config, command);

    Ok(TakeLeavePlan { greeting, farewell })
}

/// Builds a `GreetCommand` pre-populated from a farewell command.
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when greeting defaults cannot be loaded.
///
// Example usage (internal reference):
//
//     let mut farewell = TakeLeaveCommand::default();
//     farewell.greeting_preamble = Some(String::from("Mind the gap"));
//     farewell.greeting_punctuation = Some(String::from("?"));
//
//     let defaults = build_greeting_defaults(&farewell)?;
//
//     assert_eq!(defaults.preamble, Some(String::from("Mind the gap")));
//     assert_eq!(defaults.punctuation, String::from("?"));
fn build_greeting_defaults(command: &TakeLeaveCommand) -> Result<GreetCommand, HelloWorldError> {
    let mut greeting_defaults = crate::cli::load_greet_defaults()?;
    if let Some(preamble) = &command.greeting_preamble {
        greeting_defaults.preamble = Some(preamble.clone());
    }
    if let Some(punctuation) = &command.greeting_punctuation {
        greeting_defaults.punctuation.clone_from(punctuation);
    }
    Ok(greeting_defaults)
}

/// Builds optional farewell details in a stable wave, gift, channel, reminder order.
///
/// Absent options contribute no fragment, allowing the formatter to choose
/// clean punctuation for both empty and populated detail lists.
fn build_farewell_fragments(command: &TakeLeaveCommand) -> Vec<String> {
    let mut fragments = Vec::new();
    if command.wave {
        fragments.push(String::from("waves enthusiastically"));
    }
    if let Some(gift) = &command.gift {
        fragments.push(format!("leaves {}", gift.trim()));
    }
    if let Some(channel) = command.channel {
        fragments.push(format!("follows up with {}", channel.describe()));
    }
    if let Some(minutes) = command.remind_in {
        let suffix = if minutes == 1 { "" } else { "s" };
        fragments.push(format!("schedules a reminder in {minutes} minute{suffix}"));
    }
    fragments
}

/// Formats the parting phrase, recipient, and optional follow-up fragments.
///
/// With no details the sentence receives one final period; otherwise fragments
/// are joined as a grammatical list and enclosed by sentence punctuation.
fn format_farewell_message(config: &HelloWorldCli, command: &TakeLeaveCommand) -> String {
    let mut farewell = format!("{}, {}", command.parting.trim(), config.recipient);
    let fragments = build_farewell_fragments(command);
    if fragments.is_empty() {
        farewell.push('.');
    } else {
        farewell.push_str(". ");
        farewell.push_str(&join_fragments(&fragments));
        farewell.push('.');
    }
    farewell
}

/// Writes a greeting preamble, when present, followed by one newline-terminated message.
///
/// Each write is fallible and stops the sequence on the first `io::Error`.
fn write_plan_to<W: Write>(writer: &mut W, plan: &GreetingPlan) -> io::Result<()> {
    if let Some(preamble) = plan.preamble() {
        writer.write_all(preamble.as_bytes())?;
        writer.write_all(b"\n")?;
    }
    writer.write_all(plan.message().as_bytes())?;
    writer.write_all(b"\n")
}

/// Writes the embedded greeting first, then the newline-terminated farewell.
fn write_take_leave_to<W: Write>(writer: &mut W, plan: &TakeLeavePlan) -> io::Result<()> {
    write_plan_to(writer, plan.greeting())?;
    writer.write_all(plan.farewell().as_bytes())?;
    writer.write_all(b"\n")
}

/// Prints the greeting to standard output.
///
/// # Errors
///
/// Returns an [`io::Error`] when writing to standard output fails.
///
/// # Examples
///
/// ```rust
/// use hello_world::cli::{GreetCommand, HelloWorldCli};
/// use hello_world::error::HelloWorldError;
/// use hello_world::message::{build_plan, print_plan};
///
/// fn demo() -> Result<(), HelloWorldError> {
///     let globals = HelloWorldCli::default();
///     let command = GreetCommand::default();
///     let plan = build_plan(&globals, &command)?;
///     print_plan(&plan)?;
///     Ok(())
/// }
///
/// assert!(demo().is_ok());
/// ```
pub fn print_plan(plan: &GreetingPlan) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    write_plan_to(&mut stdout, plan)
}

/// Prints the farewell workflow to standard output.
///
/// # Errors
///
/// Returns an [`io::Error`] when writing to standard output fails.
///
/// # Examples
///
/// ```rust
/// use hello_world::cli::{HelloWorldCli, TakeLeaveCommand};
/// use hello_world::error::HelloWorldError;
/// use hello_world::message::{build_take_leave_plan, print_take_leave};
///
/// fn demo() -> Result<(), HelloWorldError> {
///     let globals = HelloWorldCli::default();
///     let command = TakeLeaveCommand::default();
///     let plan = build_take_leave_plan(&globals, &command)?;
///     print_take_leave(&plan)?;
///     Ok(())
/// }
///
/// assert!(demo().is_ok());
/// ```
pub fn print_take_leave(plan: &TakeLeavePlan) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    write_take_leave_to(&mut stdout, plan)
}

/// Joins detail fragments with natural conjunction and Oxford-comma handling.
///
/// Zero parts produce an empty string, one is unchanged, two use `and`, and
/// longer lists use commas followed by `and` before the final part.
fn join_fragments(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [single] => single.clone(),
        [first, second] => format!("{first} and {second}"),
        _ => match parts.split_last() {
            Some((last, rest)) => {
                let mut sentence = rest.iter().fold(String::new(), |mut acc, fragment| {
                    if !acc.is_empty() {
                        acc.push_str(", ");
                    }
                    acc.push_str(fragment);
                    acc
                });
                sentence.push_str(", and ");
                sentence.push_str(last);
                sentence
            }
            None => String::new(),
        },
    }
}

#[cfg(test)]
mod tests;
