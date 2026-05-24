//! Binds `cargo-orthohelp` feature files to the step registry.

use rstest_bdd_macros::scenarios;

use super::steps::{OrthoHelpContext, orthohelp_context};

scenarios!(
    "tests/features/orthohelp_ir.feature",
    fixtures = [orthohelp_context: OrthoHelpContext]
);
scenarios!(
    "tests/features/orthohelp_roff.feature",
    fixtures = [orthohelp_context: OrthoHelpContext]
);
scenarios!(
    "tests/features/orthohelp_powershell.feature",
    fixtures = [orthohelp_context: OrthoHelpContext]
);
