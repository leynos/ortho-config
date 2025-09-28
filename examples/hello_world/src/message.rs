//! Greeting planning and rendering for the `hello_world` example.
use crate::cli::{DeliveryMode, GreetCommand, HelloWorldCli, TakeLeaveCommand};
use crate::error::HelloWorldError;
use ortho_config::SubcmdConfigMerge;

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

fn build_greeting_defaults(command: &TakeLeaveCommand) -> Result<GreetCommand, HelloWorldError> {
    let mut greeting_defaults = GreetCommand::default().load_and_merge()?;
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

/// Prints the greeting to standard output.
pub fn print_plan(plan: &GreetingPlan) {
    if let Some(preamble) = plan.preamble() {
        println!("{preamble}");
    }
    println!("{}", plan.message());
}

/// Prints the farewell workflow to standard output.
pub fn print_take_leave(plan: &TakeLeavePlan) {
    print_plan(plan.greeting());
    println!("{}", plan.farewell());
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
    use crate::cli::FarewellChannel;
    use crate::error::ValidationError;
    use ortho_config::SubcmdConfigMerge;
    use rstest::{fixture, rstest};

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
    fn build_take_leave_plan_produces_steps(
        base_config: HelloWorldCli,
        mut take_leave_command: TakeLeaveCommand,
    ) {
        take_leave_command.wave = true;
        take_leave_command.gift = Some(String::from("biscuits"));
        take_leave_command.channel = Some(FarewellChannel::Email);
        take_leave_command.remind_in = Some(10);
        let plan = build_take_leave_plan(&base_config, &take_leave_command).expect("plan");
        assert_eq!(plan.greeting().message(), "Hello, World!");
        assert!(plan.farewell().contains("waves enthusiastically"));
        assert!(plan.farewell().contains("leaves biscuits"));
        assert!(plan.farewell().contains("follows up with an email"));
        assert!(plan.farewell().contains("10 minutes"));
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
    fn build_take_leave_plan_uses_greet_defaults(
        base_config: HelloWorldCli,
        take_leave_command: TakeLeaveCommand,
    ) {
        ortho_config::figment::Jail::expect_with(|jail| {
            jail.clear_env();
            jail.set_env("HELLO_WORLD_CMDS_GREET_PUNCTUATION", "?");
            jail.create_file(
                ".hello_world.toml",
                r#"[cmds.greet]
punctuation = "?"
"#,
            )?;
            let defaults = GreetCommand::default().load_and_merge().expect("defaults");
            let expected = build_plan(&base_config, &defaults).expect("expected greeting");
            let plan = build_take_leave_plan(&base_config, &take_leave_command).expect("plan");
            assert_eq!(plan.greeting(), &expected);
            Ok(())
        });
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
