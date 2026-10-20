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
//! The handler calls are type-checked rather than executed. `#[given]` creates
//! a wrapper for each annotated function, while `main` calls the handlers
//! directly; compiling the scratch crate checks both the generated wrappers
//! and the handlers' alias-bearing return types. A registration that stopped
//! compiling for one alias fails the scratch build, as does a handler call
//! whose return type no longer matches its alias.
//!
//! This is a pass fixture, and `trybuild` does not execute its `main`. It is
//! kept free of scenario files and of any `#[scenario]` binding: a scratch
//! crate has no feature file of its own, and runtime behaviour is covered by
//! the `rstest_bdd` integration target in `ortho_config`.
//!
//! The return annotations are deliberate: this fixture checks that both
//! aliases compile through the generated wrapper. `trybuild` compiles the
//! fixture with Cargo and `rustc`, not Clippy.

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
