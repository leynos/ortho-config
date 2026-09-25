//! Contract coverage for the documented boolean-flag override flow.
//!
//! The user's guide promises three spellings for a generated boolean flag: an
//! omitted flag defers to the lower layers, a bare flag means `true`, and
//! `--flag=false` clears a lower-precedence `true`. These assertions compile
//! and run the published `guide-boolean-override` example against each
//! spelling, so the documented contract fails the build if it drifts.

use anyhow::Result;

use crate::workspace::{EnvironmentVariable, ExampleId, ExampleWorkspace};
use crate::{Invocation, assert_run, assert_run_with_environment};

/// Asserts the boolean-flag spellings promised by the user's guide.
pub(super) fn assert_boolean_override_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
    assert_run(
        workspace,
        ExampleId("guide-boolean-override"),
        [],
        "excited=false\n",
    )?;
    assert_run(
        workspace,
        ExampleId("guide-boolean-override"),
        ["--excited"],
        "excited=true\n",
    )?;
    assert_run(
        workspace,
        ExampleId("guide-boolean-override"),
        ["--excited=false"],
        "excited=false\n",
    )?;

    assert_run_with_environment(
        workspace,
        ExampleId("guide-boolean-override"),
        Invocation {
            args: [],
            environment: [EnvironmentVariable {
                name: "ACME_EXCITED",
                value: "true",
            }],
        },
        "excited=true\n",
    )?;
    assert_run_with_environment(
        workspace,
        ExampleId("guide-boolean-override"),
        Invocation {
            args: ["--excited=false"],
            environment: [EnvironmentVariable {
                name: "ACME_EXCITED",
                value: "true",
            }],
        },
        "excited=false\n",
    )
}
