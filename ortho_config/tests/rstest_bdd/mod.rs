//! `rstest-bdd` scaffolding for `ortho_config`.
//!
//! The modules defined alongside this entrypoint register reusable fixtures,
//! the primary behavioural steps, and a canary scenario so that the
//! `rstest-bdd` macros execute under `cargo test` without needing to disable
//! the harness.

#![allow(
    clippy::doc_markdown,
    clippy::expect_used,
    clippy::if_not_else,
    clippy::missing_const_for_fn,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::redundant_closure,
    clippy::redundant_closure_for_method_calls,
    clippy::result_large_err,
    clippy::shadow_reuse,
    clippy::shadow_unrelated,
    clippy::str_to_string,
    clippy::too_many_arguments,
    clippy::trivially_copy_pass_by_ref,
    clippy::unnecessary_wraps,
    clippy::uninlined_format_args,
    clippy::unwrap_or_default,
    clippy::useless_concat,
    reason = "the BDD harness was dormant before this target was enabled; \
              cleanup belongs in a focused follow-up"
)]

/// Shared test fixtures for integration tests.
#[path = "../fixtures/mod.rs"]
pub mod fixtures;

#[path = "../support/default_punct.rs"]
mod default_punct;

mod behaviour;
mod canary;
mod canary_steps;
mod nested_docs_fixture;
mod scenario_state;
