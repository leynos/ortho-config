//! Shared support for the standalone-boundary contracts.
//!
//! Every contract in this directory runs a real Cargo command from inside
//! `ortho_l10n`'s own integration-test binary. Cargo already owns the parent
//! process, so a naive nested invocation re-enters the same package. The
//! fixtures here instead render a *foreign* package root under `target/`,
//! give it its own `[workspace]` table so it cannot inherit this one, and
//! point Cargo at a private target directory.
//!
//! The modules divide by what a contract is asking:
//!
//! - [`fixture_cargo`] owns the disposable root, its closed child environment,
//!   and the reading of Cargo's metadata document;
//! - [`consumer`] renders the package roots a stranger would write;
//! - [`package_set`] materialises the crates as `cargo package` publishes them;
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
