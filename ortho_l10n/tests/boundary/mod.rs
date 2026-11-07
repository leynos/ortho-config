//! Shared support for the standalone-boundary contracts.
//!
//! Every contract in this directory runs a real Cargo command from inside
//! `ortho_l10n`'s own integration-test binary. Cargo already owns the parent
//! process, so a naive nested invocation re-enters the same package. The
//! fixtures here instead render a *foreign* package root under `target/` and
//! give it its own `[workspace]` table so it cannot inherit this one. Cargo
//! uses the workspace target Cargo resolved, which reuses dependencies the
//! parent gate already built.
//!
//! The modules divide by what a contract is asking:
//!
//! - [`fixture_cargo`] owns the disposable root, its closed child environment,
//!   target-directory discovery, the reading of Cargo's metadata document, and
//!   a process-local gate for commands using the shared target;
//! - [`consumer`] renders the package roots a stranger would write;
//! - [`package_set`] materializes the crates as `cargo package` publishes them;
//! - [`closure`] states the removal claim and the negative controls that keep
//!   it honest.

#[path = "closure/mod.rs"]
pub(crate) mod closure;

#[path = "consumer/mod.rs"]
pub(crate) mod consumer;

#[path = "fixture_cargo/mod.rs"]
pub(crate) mod fixture_cargo;

#[path = "package_set/mod.rs"]
pub(crate) mod package_set;
