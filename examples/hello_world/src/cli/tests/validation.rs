//! Validation behaviour for CLI configuration and commands.

use super::helpers::*;
use crate::cli::{DeliveryMode, GreetCommand, HelloWorldCli, TakeLeaveCommand};
use crate::error::ValidationError;
use anyhow::{Result, anyhow, ensure};
use rstest::rstest;

#[rstest]
fn hello_world_cli_detects_conflicting_modes(base_cli: HelloWorldCliFixture) -> Result<()> {
    let mut cli = base_cli?;
    cli.is_excited = true;
    cli.is_quiet = true;
    let Err(err) = cli.validate() else {
        return Err(anyhow!(
            "expected conflicting delivery modes to fail validation"
        ));
    };
    ensure!(
        err == ValidationError::ConflictingDeliveryModes,
        "unexpected validation error: {err:?}"
    );
    Ok(())
}

#[rstest]
#[case::missing_salutations(
    |cli: &mut HelloWorldCli| {
        cli.salutations.clear();
        Ok(())
    },
    ValidationError::MissingSalutation
)]
#[case::blank_salutation(
    |cli: &mut HelloWorldCli| {
        cli.salutations.first_mut().map_or_else(
            || Err(anyhow!("expected at least one salutation")),
            |first| {
                *first = String::from("   ");
                Ok(())
            },
        )
    },
    ValidationError::BlankSalutation(0)
)]
fn hello_world_cli_validation_errors<F>(
    base_cli: HelloWorldCliFixture,
    #[case] mutate: F,
    #[case] expected: ValidationError,
) -> Result<()>
where
    F: Fn(&mut HelloWorldCli) -> Result<()>,
{
    let mut cli = base_cli?;
    mutate(&mut cli)?;
    let Err(err) = cli.validate() else {
        return Err(anyhow!("expected validation to fail with {expected:?}"));
    };
    ensure!(err == expected, "unexpected validation error: {err:?}");
    Ok(())
}

#[rstest]
#[case::excited(true, false, DeliveryMode::Enthusiastic)]
#[case::quiet(false, true, DeliveryMode::Quiet)]
#[case::standard(false, false, DeliveryMode::Standard)]
fn delivery_mode_from_flags(
    base_cli: HelloWorldCliFixture,
    #[case] excited: bool,
    #[case] quiet: bool,
    #[case] expected: DeliveryMode,
) -> Result<()> {
    let mut cli = base_cli?;
    cli.is_excited = excited;
    cli.is_quiet = quiet;
    let mode = cli.delivery_mode();
    ensure!(mode == expected, "unexpected delivery mode: {mode:?}");
    Ok(())
}

#[rstest]
fn trimmed_salutations_remove_whitespace(base_cli: HelloWorldCliFixture) -> Result<()> {
    let mut cli = base_cli?;
    cli.salutations = vec![String::from("  Hi"), String::from("Team  ")];
    let expected = vec![String::from("Hi"), String::from("Team")];
    ensure!(
        cli.trimmed_salutations() == expected,
        "expected trimmed salutations"
    );
    Ok(())
}

#[rstest]
#[case::punctuation(
    |command: &mut GreetCommand| {
        command.punctuation = String::from("   ");
        Ok(())
    },
    ValidationError::BlankPunctuation,
    "greeting punctuation must contain visible characters",
)]
#[case::preamble(
    |command: &mut GreetCommand| {
        command.preamble = Some(String::from("   "));
        Ok(())
    },
    ValidationError::BlankPreamble,
    "preambles must contain visible characters when supplied",
)]
fn greet_command_rejects_blank_inputs<F>(
    greet_command: GreetCommandFixture,
    #[case] mutate: F,
    #[case] expected_error: ValidationError,
    #[case] expected_message: &str,
) -> Result<()>
where
    F: Fn(&mut GreetCommand) -> Result<()>,
{
    let mut command = greet_command?;
    mutate(&mut command)?;
    let Err(err) = command.validate() else {
        return Err(anyhow!("expected validation to fail"));
    };
    ensure!(
        err == expected_error,
        "unexpected validation error: {err:?}"
    );
    ensure!(
        err.to_string() == expected_message,
        "unexpected validation message"
    );
    Ok(())
}

#[rstest]
#[case::blank_parting(
    |cmd: &mut TakeLeaveCommand| {
        cmd.parting = String::from(" ");
        Ok(())
    },
    ValidationError::BlankFarewell,
    "farewell messages must contain visible characters"
)]
#[case::zero_reminder(
    |cmd: &mut TakeLeaveCommand| {
        cmd.remind_in = Some(0);
        Ok(())
    },
    ValidationError::ReminderOutOfRange,
    "reminder minutes must be greater than zero"
)]
#[case::blank_gift(
    |cmd: &mut TakeLeaveCommand| {
        cmd.gift = Some(String::from("   "));
        Ok(())
    },
    ValidationError::BlankGift,
    "gift descriptions must contain visible characters"
)]
#[case::blank_greeting_preamble(
    |cmd: &mut TakeLeaveCommand| {
        cmd.greeting_preamble = Some(String::from("   "));
        Ok(())
    },
    ValidationError::BlankPreamble,
    "preambles must contain visible characters when supplied"
)]
#[case::blank_greeting_punctuation(
    |cmd: &mut TakeLeaveCommand| {
        cmd.greeting_punctuation = Some(String::from("   "));
        Ok(())
    },
    ValidationError::BlankPunctuation,
    "greeting punctuation must contain visible characters"
)]
fn take_leave_command_validation_errors<F>(
    take_leave_command: TakeLeaveCommandFixture,
    #[case] setup: F,
    #[case] expected_error: ValidationError,
    #[case] expected_message: &str,
) -> Result<()>
where
    F: Fn(&mut TakeLeaveCommand) -> Result<()>,
{
    let mut command = take_leave_command?;
    setup(&mut command)?;
    let Err(err) = command.validate() else {
        return Err(anyhow!("expected validation to fail"));
    };
    ensure!(
        err == expected_error,
        "unexpected validation error: {err:?}"
    );
    ensure!(
        err.to_string() == expected_message,
        "unexpected validation message"
    );
    Ok(())
}
