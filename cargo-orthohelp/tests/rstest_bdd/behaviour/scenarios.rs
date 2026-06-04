//! Scenario registration for `cargo-orthohelp` behavioural tests.
//!
//! Wires each Gherkin feature file to the rstest-bdd step registry by calling
//! `scenarios!` with the feature-file path and the [`orthohelp_context`]
//! fixture. Four feature files are registered:
//!
//! - `tests/features/orthohelp_ir.feature` — IR JSON generation and caching.
//! - `tests/features/orthohelp_roff.feature` — roff man-page generation.
//! - `tests/features/orthohelp_powershell.feature` — `PowerShell` help
//!   generation.
//! - `tests/features/orthohelp_agent_context.feature` — agent-context JSON
//!   generation.
//!
//! The `fixtures = [orthohelp_context: OrthoHelpContext]` binding tells the
//! framework to call [`orthohelp_context`] once per scenario to produce the
//! initial [`OrthoHelpContext`] value, which is then threaded through every
//! step function as `&mut OrthoHelpContext`.

use rstest_bdd_macros::scenarios;

use super::steps::{OrthoHelpContext, orthohelp_context};

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
