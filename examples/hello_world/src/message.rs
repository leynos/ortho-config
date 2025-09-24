//! Greeting planning and rendering for the `hello_world` example.
use crate::cli::{DeliveryMode, GreetCommand, HelloWorldCli, TakeLeaveCommand};
use crate::error::HelloWorldError;

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
    let greeting = build_plan(config, &GreetCommand::default())?;
    let mut farewell = format!("{}, {}", command.parting.trim(), config.recipient);
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
    if fragments.is_empty() {
        farewell.push('.');
    } else {
        farewell.push_str(". ");
        farewell.push_str(&join_fragments(&fragments));
        farewell.push('.');
    }
    Ok(TakeLeavePlan { greeting, farewell })
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
    use crate::error::ValidationError;
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
        take_leave_command.remind_in = Some(10);
        let plan = build_take_leave_plan(&base_config, &take_leave_command).expect("plan");
        assert_eq!(plan.greeting().message(), "Hello, World!");
        assert!(plan.farewell().contains("waves enthusiastically"));
        assert!(plan.farewell().contains("leaves biscuits"));
        assert!(plan.farewell().contains("10 minutes"));
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
    }
}
