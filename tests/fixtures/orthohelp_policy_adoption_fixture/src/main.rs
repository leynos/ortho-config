//! Policy adoption fixture for `cargo-orthohelp` integration tests.
//!
//! Deliberately lacks every generator precondition: no `root_type` metadata,
//! no `ortho_config` dependency, and no library target. The agent-native
//! policy check must therefore still run and write its report before generator
//! package selection and bridge construction, per `ExecPlan` Decision D11 and
//! acceptance criterion 6.
//!
//! A binary target exists only because a package with no targets cannot be a
//! workspace member; nothing runs it.

fn main() {}
