//! Policy adoption fixture for `cargo-orthohelp` integration tests.
//!
//! Deliberately lacks the generator preconditions (`root_type` metadata and
//! an `ortho_config` dependency) so the agent-native policy check can prove
//! it runs before generator package selection and bridge construction, per
//! `ExecPlan` Decision D11 and acceptance criterion 6.
