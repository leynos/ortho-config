//! Behavioural test harness for `cargo-orthohelp`.
//!
//! This module is the root of the BDD step registry. It declares the
//! submodules that together cover all Gherkin scenarios under
//! `cargo-orthohelp/tests/features/`:
//!
//! - [`steps`] — shared scenario state ([`steps::OrthoHelpContext`]), the
//!   rstest fixture, and target-directory helpers.
//! - [`steps_cmd`] — `given`/`when` steps that invoke `cargo-orthohelp` and
//!   manage the temporary output directory.
//! - [`steps_cache`] — `then` steps that verify cache behaviour (identity,
//!   schema version, missing-cache failure).
//! - [`steps_ir`] — `then` steps that assert IR JSON content and locale
//!   correctness.
//! - [`roff_steps`] — `when`/`then` steps for roff man-page generation.
//! - [`powershell_steps`] — `when`/`then` steps for `PowerShell` help
//!   generation.
//! - [`steps_agent_context`] — `when`/`then` steps for agent-context JSON
//!   generation.
//! - [`scenarios`] — wires each feature file to the step registry via
//!   `scenarios!`.

mod powershell_steps;
mod roff_steps;
mod scenarios;
pub mod steps;
mod steps_agent_context;
mod steps_cache;
mod steps_cmd;
mod steps_ir;
