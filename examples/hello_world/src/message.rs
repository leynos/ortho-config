//! Greeting planning and rendering for the `hello_world` example.
use crate::cli::{DeliveryMode, HelloWorldCli};
use crate::error::HelloWorldError;

/// Computed greeting ready for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreetingPlan {
    message: String,
    mode: DeliveryMode,
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
}

/// Builds a [`GreetingPlan`] from the resolved configuration.
pub fn build_plan(config: &HelloWorldCli) -> Result<GreetingPlan, HelloWorldError> {
    config.validate()?;
    let mode = config.delivery_mode();
    let salutation = config.trimmed_salutations().join(" ");
    let recipient = &config.recipient;
    let base = format!("{salutation}, {recipient}");
    let message = match mode {
        DeliveryMode::Standard => format!("{base}!"),
        DeliveryMode::Enthusiastic => format!("{}!", base.to_uppercase()),
        DeliveryMode::Quiet => format!("{base}..."),
    };
    Ok(GreetingPlan { message, mode })
}

/// Prints the greeting to standard output.
pub fn print_plan(plan: &GreetingPlan) {
    println!("{}", plan.message());
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

    #[rstest]
    fn build_plan_produces_default_message(base_config: HelloWorldCli) {
        let plan = build_plan(&base_config).expect("plan");
        assert_eq!(plan.mode(), DeliveryMode::Standard);
        assert_eq!(plan.message(), "Hello, World!");
    }

    #[rstest]
    fn build_plan_shouts_for_excited(mut base_config: HelloWorldCli) {
        base_config.is_excited = true;
        let plan = build_plan(&base_config).expect("plan");
        assert_eq!(plan.mode(), DeliveryMode::Enthusiastic);
        assert_eq!(plan.message(), "HELLO, WORLD!");
    }

    #[rstest]
    fn build_plan_whispers_for_quiet(mut base_config: HelloWorldCli) {
        base_config.is_quiet = true;
        let plan = build_plan(&base_config).expect("plan");
        assert_eq!(plan.mode(), DeliveryMode::Quiet);
        assert_eq!(plan.message(), "Hello, World...");
    }

    #[rstest]
    fn build_plan_propagates_validation_errors(mut base_config: HelloWorldCli) {
        base_config.salutations.clear();
        let err = build_plan(&base_config).expect_err("invalid plan");
        assert!(
            matches!(
                err,
                HelloWorldError::Validation(ValidationError::MissingSalutation)
            ),
            "expected missing salutation error",
        );
    }
}
