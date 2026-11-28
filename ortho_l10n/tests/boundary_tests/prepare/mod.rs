//! The preparation step, run by `make boundary-prepare` and by nothing else.
//!
//! The timed contracts read packaged roots and build consumer roots against a
//! shared target directory. Both used to start cold inside the contract's own
//! allowance: `cargo package --verify` compiles the packaged closure on every
//! run, and each consumer compiled the dependency tree it shares with the
//! others. On a shared host the compile-admission queue outlasted the
//! allowance while the code under test was sound (#700, #755).
//!
//! This step pays that cost once, outside every timed test:
//!
//! 1. both verified `cargo package` invocations, with their patches and
//!    without `--no-verify`, then the preparation record the contracts check;
//! 2. `cargo test --no-run --all-targets --all-features` for every consumer
//!    shape against both sources, so the shared consumer target holds the
//!    dependency closure and a timed contract compiles only its own crate.
//!
//! The roots are still produced by `cargo package`, and the consumers are still
//! foreign workspaces rendered afresh for each contract: nothing the contracts
//! assert is replaced, only moved ahead of the clock.
//!
//! The test is `#[ignore]`d so `cargo test` and nextest never run it as part of
//! the suite. `make boundary-prepare` runs it with `--ignored`, and `make test`
//! and `make boundary` depend on that target. A direct `cargo test` of the
//! binary without it fails the packaged contracts with a precondition that
//! names `make boundary-prepare`.

use crate::boundary::consumer::{Consumer, Dependency, DependencySource};
use crate::boundary::fixture_cargo::Budget;
use crate::boundary::package_set::{ConsumerFixture, package_and_record, unpacked_directory};
use anyhow::{Context, Result};

/// Package and verify the set, record it, and warm every consumer build.
///
/// No per-call deadline applies: [`Budget::preparation`] bounds each call by a
/// hang guard only. A failure carries the failing invocation's own output.
#[test]
#[ignore = "the preparation step: run by `make boundary-prepare`, not by the timed suite"]
fn prepare_boundary_fixtures() -> Result<()> {
    let budget = Budget::preparation();
    package_and_record(budget)?;
    let packaged = DependencySource::Packaged {
        unpacked: unpacked_directory()?,
    };
    prepare_consumers(&DependencySource::Workspace, budget)?;
    prepare_consumers(&packaged, budget)
}

/// Build the tests of every consumer shape resolved from `source`.
fn prepare_consumers(source: &DependencySource, budget: Budget) -> Result<()> {
    Dependency::ALL
        .into_iter()
        .try_for_each(|dependency| prepare_consumer(dependency, source, budget))
}

/// Render one consumer in a fresh foreign root and build its tests.
fn prepare_consumer(
    dependency: Dependency,
    source: &DependencySource,
    budget: Budget,
) -> Result<()> {
    let shape = format!(
        "the {} consumer from {} sources",
        dependency.describe(),
        source.describe(),
    );
    let consumer = Consumer::render(dependency, source).with_context(|| shape.clone())?;
    let fixture = ConsumerFixture::new(budget)?.write_consumer(&consumer)?;
    fixture.build().with_context(|| format!("prepare {shape}"))
}
