//! CLI configuration for the `hello_world` example.
//!
//! Binds CLI, environment, and default layers via `OrthoConfig` so tests can
//! drive the binary with predictable inputs.
use crate::error::ValidationError;
use ortho_config::OrthoConfig;
use serde::Deserialize;

/// Top-level configuration for the hello world demo.
///
/// The struct collects the global options exposed by the example, keeping
/// fields public so the command dispatcher can inspect the resolved values
/// without extra accessor boilerplate.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "HELLO_WORLD")]
pub struct HelloWorldCli {
    /// Recipient of the greeting. Defaults to a friendly placeholder.
    #[serde(default = "default_recipient")]
    #[ortho_config(default = default_recipient(), cli_short = 'r')]
    pub recipient: String,
    /// Words used to open the greeting. Demonstrates repeated parameters.
    #[serde(default = "default_salutations")]
    #[ortho_config(default = default_salutations(), cli_short = 's')]
    pub salutations: Vec<String>,
    /// Enables an enthusiastic delivery mode.
    #[serde(default)]
    #[ortho_config(default = false)]
    pub is_excited: bool,
    /// Selects a quiet delivery mode.
    #[serde(default)]
    #[ortho_config(default = false)]
    pub is_quiet: bool,
}

impl Default for HelloWorldCli {
    fn default() -> Self {
        Self {
            recipient: default_recipient(),
            salutations: default_salutations(),
            is_excited: false,
            is_quiet: false,
        }
    }
}

/// Delivery strategy derived from the global switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    /// Standard delivery keeps the message as-is.
    Standard,
    /// Enthusiastic delivery shouts the greeting.
    Enthusiastic,
    /// Quiet delivery whispers the message.
    Quiet,
}

impl HelloWorldCli {
    #[inline]
    fn has_conflicting_modes(&self) -> bool {
        self.is_excited && self.is_quiet
    }

    /// Validates the resolved configuration before execution.
    ///
    /// # Examples
    /// ```ignore
    /// # use hello_world::cli::{DeliveryMode, HelloWorldCli};
    /// let mut cli = HelloWorldCli::default();
    /// cli.is_excited = true;
    /// cli.validate().unwrap();
    /// assert_eq!(cli.delivery_mode(), DeliveryMode::Enthusiastic);
    /// ```
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.has_conflicting_modes() {
            return Err(ValidationError::ConflictingDeliveryModes);
        }
        if self.salutations.is_empty() {
            return Err(ValidationError::MissingSalutation);
        }
        for (index, word) in self.salutations.iter().enumerate() {
            if word.trim().is_empty() {
                return Err(ValidationError::BlankSalutation(index));
            }
        }
        Ok(())
    }

    /// Calculates the delivery mode once the configuration is valid.
    #[must_use]
    pub fn delivery_mode(&self) -> DeliveryMode {
        debug_assert!(
            !self.has_conflicting_modes(),
            "Call validate() before delivery_mode(); conflicting flags set",
        );
        match (self.is_excited, self.is_quiet) {
            (true, false) => DeliveryMode::Enthusiastic,
            (false, true) => DeliveryMode::Quiet,
            _ => DeliveryMode::Standard,
        }
    }

    /// Strips incidental whitespace from salutations for consistent output.
    ///
    /// # Examples
    /// ```ignore
    /// # use hello_world::cli::HelloWorldCli;
    /// let mut cli = HelloWorldCli::default();
    /// cli.salutations = vec!["  Hello".into(), "world  ".into()];
    /// assert_eq!(cli.trimmed_salutations(), vec!["Hello", "world"]);
    /// ```
    #[must_use]
    pub fn trimmed_salutations(&self) -> Vec<String> {
        self.salutations
            .iter()
            .map(|word| String::from(word.trim()))
            .collect()
    }
}

fn default_recipient() -> String {
    String::from("World")
}

fn default_salutations() -> Vec<String> {
    vec![String::from("Hello")]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn base_cli() -> HelloWorldCli {
        HelloWorldCli::default()
    }

    #[rstest]
    fn default_configuration_is_valid(base_cli: HelloWorldCli) {
        base_cli.validate().expect("default config is valid");
    }

    #[rstest]
    fn conflicting_delivery_modes_are_rejected(mut base_cli: HelloWorldCli) {
        base_cli.is_excited = true;
        base_cli.is_quiet = true;
        let err = base_cli
            .validate()
            .expect_err("conflicting modes should fail");
        assert_eq!(err, ValidationError::ConflictingDeliveryModes);
    }

    #[rstest]
    fn missing_salutation_is_rejected(mut base_cli: HelloWorldCli) {
        base_cli.salutations.clear();
        let err = base_cli
            .validate()
            .expect_err("missing salutation should fail");
        assert_eq!(err, ValidationError::MissingSalutation);
    }

    #[rstest]
    fn blank_salutation_is_rejected(mut base_cli: HelloWorldCli) {
        base_cli.salutations[0] = String::from("   ");
        let err = base_cli
            .validate()
            .expect_err("blank salutation should fail");
        assert_eq!(err, ValidationError::BlankSalutation(0));
    }

    #[rstest]
    #[case(false, false, DeliveryMode::Standard)]
    #[case(true, false, DeliveryMode::Enthusiastic)]
    #[case(false, true, DeliveryMode::Quiet)]
    fn delivery_mode_resolves_preference(
        mut base_cli: HelloWorldCli,
        #[case] is_excited: bool,
        #[case] is_quiet: bool,
        #[case] expected: DeliveryMode,
    ) {
        base_cli.is_excited = is_excited;
        base_cli.is_quiet = is_quiet;
        assert_eq!(base_cli.delivery_mode(), expected);
    }

    #[rstest]
    fn trimmed_salutations_strip_whitespace(mut base_cli: HelloWorldCli) {
        base_cli.salutations = vec![String::from("  Hello"), String::from("world  ")];
        let trimmed = base_cli.trimmed_salutations();
        assert_eq!(trimmed, vec![String::from("Hello"), String::from("world")],);
    }
}
