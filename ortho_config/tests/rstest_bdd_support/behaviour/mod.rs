//! Behavioural test harness for `ortho_config` using `rstest-bdd`.
//!
//! Step implementations live under [`steps`], while [`scenarios`] binds the
//! existing `.feature` files to the shared fixtures.

mod scenarios;
pub mod steps;
