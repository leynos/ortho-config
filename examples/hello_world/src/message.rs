//! Greeting planning and rendering for the `hello_world` example.
use crate::cli::{DeliveryMode, GreetCommand, HelloWorldCli, TakeLeaveCommand};
use crate::error::HelloWorldError;
use std::io::{self, Write};

/// Computed greeting ready for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreetingPlan {
    message: String,
    mode: DeliveryMode,
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
    pub fn mode(&self) -> DeliveryMode {
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
    greeting: GreetingPlan,
    farewell: String,
}

impl TakeLeavePlan {
    /// Returns the embedded greeting.
    #[must_use]
    pub fn greeting(&self) -> &GreetingPlan {
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
/// use hello_world::message::build_plan;
///
/// let mut config = HelloWorldCli::default();
/// config.recipient = String::from("Ada Lovelace");
/// config.salutations = vec![String::from("Hello")];
///
/// let command = GreetCommand::default();
/// let plan = build_plan(&config, &command).expect("plan builds");
///
/// assert_eq!(plan.message(), "Hello, Ada Lovelace!");
/// assert_eq!(plan.preamble(), None);
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
/// ```
/// use hello_world::cli::{HelloWorldCli, TakeLeaveCommand};
/// use hello_world::message::build_take_leave_plan;
///
/// let mut config = HelloWorldCli::default();
/// config.recipient = String::from("Ada Lovelace");
/// config.salutations = vec![String::from("Hello there")];
///
/// let mut command = TakeLeaveCommand::default();
/// command.parting = String::from("Cheerio");
/// command.greeting_preamble = Some(String::from("Before we go"));
/// command.greeting_punctuation = Some(String::from("?"));
///
/// let plan =
///     build_take_leave_plan(&config, &command).expect("valid farewell inputs");
///
/// assert_eq!(plan.greeting().preamble(), Some("Before we go"));
/// assert_eq!(
///     plan.greeting().message(),
///     "Hello there, Ada Lovelace?"
/// );
/// assert_eq!(plan.farewell(), "Cheerio, Ada Lovelace.");
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
/// # Examples
///
/// ```
/// use hello_world::cli::{GreetCommand, TakeLeaveCommand};
/// use hello_world::message::build_greeting_defaults;
///
/// let mut farewell = TakeLeaveCommand::default();
/// farewell.greeting_preamble = Some(String::from("Mind the gap"));
/// farewell.greeting_punctuation = Some(String::from("?"));
///
/// let defaults =
///     build_greeting_defaults(&farewell).expect("defaults load successfully");
///
/// assert_eq!(
///     defaults.preamble,
///     Some(String::from("Mind the gap"))
/// );
/// assert_eq!(defaults.punctuation, String::from("?"));
/// ```
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

fn write_plan_to<W: Write>(writer: &mut W, plan: &GreetingPlan) -> io::Result<()> {
    if let Some(preamble) = plan.preamble() {
        writer.write_all(preamble.as_bytes())?;
        writer.write_all(b"\n")?;
    }
    writer.write_all(plan.message().as_bytes())?;
    writer.write_all(b"\n")
}

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
/// ```
/// use hello_world::cli::{GreetCommand, HelloWorldCli};
/// use hello_world::message::{build_plan, print_plan};
///
/// let globals = HelloWorldCli::default();
/// let command = GreetCommand::default();
/// let plan = build_plan(&globals, &command).expect("plan builds");
/// print_plan(&plan).expect("stdout write succeeds");
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
/// ```
/// use hello_world::cli::{HelloWorldCli, TakeLeaveCommand};
/// use hello_world::message::{build_take_leave_plan, print_take_leave};
///
/// let globals = HelloWorldCli::default();
/// let command = TakeLeaveCommand::default();
/// let plan = build_take_leave_plan(&globals, &command).expect("plan builds");
/// print_take_leave(&plan).expect("stdout write succeeds");
/// ```
pub fn print_take_leave(plan: &TakeLeavePlan) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    write_take_leave_to(&mut stdout, plan)
}

fn join_fragments(parts: &[String]) -> String {
    match parts.len() {
        0 => String::new(),
        1 => parts[0].clone(),
        2 => format!("{} and {}", parts[0], parts[1]),
        _ => {
            let mut sentence = parts[..parts.len() - 1].join(", ");
            sentence.push_str(", and ");
            sentence.push_str(parts.last().expect("at least one fragment"));
            sentence
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{FarewellChannel, GlobalArgs};
    use crate::error::ValidationError;
    use camino::Utf8PathBuf;
    use rstest::{fixture, rstest};

    // Helper function for setting up test environment with sample configs
    fn setup_sample_config_environment() -> HelloWorldCli {
        with_sample_config_environment(HelloWorldCli::clone)
    }

    fn with_sample_config_environment<R>(action: impl FnOnce(&HelloWorldCli) -> R) -> R {
        let mut result = None;
        ortho_config::figment::Jail::expect_with(|jail| {
            jail.clear_env();
            let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let config_dir = cap_std::fs::Dir::open_ambient_dir(
                manifest_dir.join("config").as_std_path(),
                cap_std::ambient_authority(),
            )
            .expect("open hello_world sample config directory");
            let baseline = config_dir
                .read_to_string("baseline.toml")
                .expect("read baseline sample configuration");
            let overrides = config_dir
                .read_to_string("overrides.toml")
                .expect("read overrides sample configuration");
            jail.create_file("baseline.toml", &baseline)?;
            jail.create_file(".hello_world.toml", &overrides)?;
            let config = crate::cli::load_global_config(&GlobalArgs::default(), None)
                .expect("load global config");
            result = Some(action(&config));
            Ok(())
        });
        result.expect("sample config action")
    }

    // Generic helper for testing plan building with sample configs
    fn test_sample_config_plan<P, F>(plan_builder: F, plan_validator: impl FnOnce(&P))
    where
        F: FnOnce(&HelloWorldCli) -> Result<P, HelloWorldError>,
    {
        setup_sample_config_environment();
        let plan = with_sample_config_environment(|config| plan_builder(config).expect("plan"));
        plan_validator(&plan);
    }

    #[fixture]
    fn base_config() -> HelloWorldCli {
        HelloWorldCli::default()
    }

    #[fixture]
    fn greet_command() -> GreetCommand {
        GreetCommand::default()
    }

    #[fixture]
    fn take_leave_command() -> TakeLeaveCommand {
        TakeLeaveCommand::default()
    }

    #[rstest]
    fn build_plan_produces_default_message(
        base_config: HelloWorldCli,
        greet_command: GreetCommand,
    ) {
        let plan = build_plan(&base_config, &greet_command).expect("plan");
        assert_eq!(plan.mode(), DeliveryMode::Standard);
        assert_eq!(plan.message(), "Hello, World!");
        assert_eq!(plan.preamble(), None);
    }

    #[rstest]
    fn build_plan_shouts_for_excited(mut base_config: HelloWorldCli, greet_command: GreetCommand) {
        base_config.is_excited = true;
        let plan = build_plan(&base_config, &greet_command).expect("plan");
        assert_eq!(plan.mode(), DeliveryMode::Enthusiastic);
        assert_eq!(plan.message(), "HELLO, WORLD!");
    }

    #[rstest]
    fn build_plan_applies_preamble(mut greet_command: GreetCommand, base_config: HelloWorldCli) {
        greet_command.preamble = Some(String::from("Good morning"));
        let plan = build_plan(&base_config, &greet_command).expect("plan");
        assert_eq!(plan.preamble(), Some("Good morning"));
    }

    #[rstest]
    fn build_plan_propagates_validation_errors(mut base_config: HelloWorldCli) {
        base_config.salutations.clear();
        let err = build_plan(&base_config, &GreetCommand::default()).expect_err("invalid plan");
        assert!(
            matches!(
                err,
                HelloWorldError::Validation(ValidationError::MissingSalutation)
            ),
            "expected missing salutation error",
        );
    }

    #[rstest]
    fn build_take_leave_plan_produces_steps() {
        ortho_config::figment::Jail::expect_with(|jail| {
            jail.clear_env();
            let take_leave_command = TakeLeaveCommand {
                wave: true,
                gift: Some(String::from("biscuits")),
                channel: Some(FarewellChannel::Email),
                remind_in: Some(10),
                ..TakeLeaveCommand::default()
            };
            let plan = build_take_leave_plan(&HelloWorldCli::default(), &take_leave_command)
                .expect("plan");
            assert_eq!(plan.greeting().message(), "Hello, World!");
            assert!(plan.farewell().contains("waves enthusiastically"));
            assert!(plan.farewell().contains("leaves biscuits"));
            assert!(plan.farewell().contains("follows up with an email"));
            assert!(plan.farewell().contains("10 minutes"));
            Ok(())
        });
    }

    #[rstest]
    fn build_take_leave_plan_applies_greeting_overrides(
        base_config: HelloWorldCli,
        mut take_leave_command: TakeLeaveCommand,
    ) {
        take_leave_command.greeting_preamble = Some(String::from("Until next time"));
        take_leave_command.greeting_punctuation = Some(String::from("?"));
        let plan = build_take_leave_plan(&base_config, &take_leave_command).expect("plan");
        assert_eq!(plan.greeting().preamble(), Some("Until next time"));
        assert!(plan.greeting().message().ends_with('?'));
    }

    #[rstest]
    fn build_take_leave_plan_uses_greet_defaults() {
        test_sample_config_plan(
            |config| build_take_leave_plan(config, &TakeLeaveCommand::default()),
            |plan| {
                assert_eq!(plan.greeting().preamble(), Some("Layered hello"));
                assert_eq!(
                    plan.greeting().message(),
                    "HELLO HEY CONFIG FRIENDS, EXCITED CREW!!!"
                );
                assert_eq!(plan.greeting().mode(), DeliveryMode::Enthusiastic);
            },
        );
    }

    #[rstest]
    fn build_plan_uses_sample_overrides() {
        test_sample_config_plan(
            |config| {
                let greet = crate::cli::load_greet_defaults().expect("load greet defaults");
                build_plan(config, &greet)
            },
            |plan| {
                assert_eq!(plan.preamble(), Some("Layered hello"));
                assert_eq!(plan.message(), "HELLO HEY CONFIG FRIENDS, EXCITED CREW!!!");
                assert_eq!(plan.mode(), DeliveryMode::Enthusiastic);
            },
        );
    }

    #[rstest]
    fn join_fragments_writes_list() {
        let parts = vec![
            String::from("waves"),
            String::from("leaves biscuits"),
            String::from("follows up with an email"),
        ];
        assert_eq!(
            join_fragments(&parts),
            "waves, leaves biscuits, and follows up with an email"
        );

        let pair = vec![String::from("waves"), String::from("leaves biscuits")];
        assert_eq!(join_fragments(&pair), "waves and leaves biscuits");
    }
}
