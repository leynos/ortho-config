//! Scenario registration for `cargo-orthohelp` behavioural tests.
//!
//! Wires each Gherkin feature file to the rstest-bdd step registry by calling
//! `scenarios!` with the feature-file path and its fixture. Eight feature
//! files are registered:
//!
//! - `tests/features/orthohelp_ir.feature` — IR JSON generation and caching.
//! - `tests/features/orthohelp_roff.feature` — roff man-page generation.
//! - `tests/features/orthohelp_powershell.feature` — `PowerShell` help
//!   generation.
//! - `tests/features/orthohelp_agent_context.feature` — agent-context JSON
//!   generation.
//! - `tests/features/orthohelp_policy.feature` — agent-native policy check.
//! - `tests/features/orthohelp_policy_report.feature` — agent-native policy
//!   report generation.
//! - `tests/features/structured_output.feature` — declared structured-output
//!   lint, checked in process rather than through the CLI.
//! - `tests/features/profile_redaction.feature` — profile-field disclosure
//!   boundary.
//!
//! The `fixtures = [orthohelp_context: OrthoHelpContext]` binding tells the
//! framework to call [`orthohelp_context`] once per scenario to produce the
//! initial [`OrthoHelpContext`] value, which is then threaded through every
//! step function as `&mut OrthoHelpContext`. The structured-output scenarios
//! use their own `output_policy_state` fixture because they never invoke the
//! binary.

use rstest_bdd_macros::scenarios;

use super::steps::{OrthoHelpContext, orthohelp_context};
use crate::rstest_bdd_support::output_policy::{OutputPolicyState, output_policy_state};

scenarios!(
    "tests/features/orthohelp_ir.feature",
    fixtures = [orthohelp_context: OrthoHelpContext]
);
scenarios!(
    "tests/features/orthohelp_roff.feature",
    fixtures = [orthohelp_context: OrthoHelpContext]
);
scenarios!(
    "tests/features/orthohelp_powershell.feature",
    fixtures = [orthohelp_context: OrthoHelpContext]
);
scenarios!(
    "tests/features/orthohelp_agent_context.feature",
    fixtures = [orthohelp_context: OrthoHelpContext]
);
scenarios!(
    "tests/features/orthohelp_policy.feature",
    fixtures = [orthohelp_context: OrthoHelpContext]
);
scenarios!(
    "tests/features/orthohelp_policy_report.feature",
    fixtures = [orthohelp_context: OrthoHelpContext]
);
scenarios!(
    "tests/features/structured_output.feature",
    fixtures = [output_policy_state: OutputPolicyState]
);
scenarios!(
    "tests/features/profile_redaction.feature",
    fixtures = [orthohelp_context: OrthoHelpContext]
);
