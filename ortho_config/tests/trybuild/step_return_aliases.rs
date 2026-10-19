//! Compiles the `rstest-bdd` step macros for this workspace's return aliases.
//!
//! `ortho_config` declares its steps as `anyhow::Result<T>` and
//! `cargo-orthohelp` declares its steps as
//! `Result<T, Box<dyn std::error::Error + Send + Sync>>`. `rstest-bdd` 0.6.0
//! selects a step's return normalization from the *type* the handler returns,
//! so both aliases have to reach the same `Result` arm through the sealed
//! `StepReturnNormalize` trait. That is a trait-resolution question the runtime
//! tests cannot answer: a change that stopped resolving either alias would
//! surface here as a compile error, not as a failing assertion.
//!
//! The handlers are exercised rather than merely declared. `#[given]` is a
//! no-op for a body that returns a plain value, and a step wrapper is created
//! only for the annotated function, so invoking the functions directly runs
//! the same return values through the same `Result` types the wrapper will
//! normalize. A registration that stopped compiling for one alias fails the
//! scratch build; a body that stopped type-checking for the other fails the
//! `main` below.
//!
//! This is a pass fixture, so `main` must also exit successfully. It is kept
//! free of scenario files and of any `#[scenario]` binding: a scratch crate
//! has no feature file of its own, and the runtime behaviour is covered by the
//! `rstest_bdd` integration target in `ortho_config`.
//!
//! The return annotations are deliberate. `Ok(())` from `anyhow::Result<()>`
//! is `unnecessary_wraps` for Clippy, and both aliases exist to exercise the
//! wrapper, so the expectation is scoped to the two functions that need it.

use std::error::Error;

use rstest_bdd_macros::{given, then};

/// The level payload carried out of a step.
#[derive(Debug, PartialEq, Eq)]
struct Level(u8);

/// This workspace's primary step return alias.
type StepAlias<T> = anyhow::Result<T>;

/// The boxed trait-object error `cargo-orthohelp` uses in its steps.
type BoxedStepError = Box<dyn Error + Send + Sync>;

/// The boxed error alias `cargo-orthohelp`'s steps return.
type BoxedStepResult<T> = Result<T, BoxedStepError>;

/// A failing `anyhow::Result` step, as `ortho_config` writes them.
#[given("a downstream alias reports failure")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the step deliberately returns Result to exercise the wrapper's \
              Result normalization"
)]
fn anyhow_alias_reports_failure() -> StepAlias<()> {
    Err(anyhow::anyhow!("downstream alias failed"))
}

/// A payload-carrying `anyhow::Result` step.
#[given("a downstream alias reports a level of {level:u8}")]
fn anyhow_alias_reports_level(level: u8) -> StepAlias<Level> { Ok(Level(level)) }

/// A failing boxed-error step, as `cargo-orthohelp` writes them.
#[given("a boxed downstream alias reports failure")]
fn boxed_alias_reports_failure() -> BoxedStepResult<()> {
    Err("boxed downstream alias failed".into())
}

/// A payload-carrying boxed-error step.
#[given("a boxed downstream alias reports a level of {level:u8}")]
fn boxed_alias_reports_level(level: u8) -> BoxedStepResult<Level> { Ok(Level(level)) }

/// A unit-returning `anyhow::Result` step, whose `Ok(())` carries no payload.
#[then("a downstream alias finishes with no payload")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the step deliberately returns Result to exercise the wrapper's \
              Result normalization"
)]
fn anyhow_alias_finishes_empty() -> StepAlias<()> { Ok(()) }

/// Runs each alias through the return values its wrapper must normalize.
fn main() {
    assert!(anyhow_alias_reports_failure().is_err());
    assert!(boxed_alias_reports_failure().is_err());
    assert!(anyhow_alias_finishes_empty().is_ok());

    assert_eq!(
        anyhow_alias_reports_level(7).ok(),
        Some(Level(7)),
        "an aliased Ok must carry the inner payload"
    );
    assert_eq!(
        boxed_alias_reports_level(3).ok(),
        Some(Level(3)),
        "a boxed-error Ok must carry the inner payload"
    );
}
