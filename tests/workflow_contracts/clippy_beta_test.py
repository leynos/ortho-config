"""Contract for the advisory Clippy-on-beta lane.

``clippy-beta`` exists to see a lint before it reaches a pinned toolchain.
It is advisory: ``continue-on-error`` keeps a new beta lint from blocking a
merge, while the pinned-toolchain ``build-test`` job stays required.

Advisory is only safe if the job still *sees* the code. On 2026-09-28 the
job failed on four lints inside ``cargo-orthohelp/tests/powershell_windows.rs``
— a ``Result`` predicate and three constant-size chunk iterators — that a
Linux-only leg cannot compile at all: the file is gated behind
``cfg(windows)``, as is the MSVC branch of
``ortho_config/tests/documentation_examples/cargo_runner.rs``. An advisory
lane that never compiles a file cannot warn about it, so the coverage is
half the contract and the strictness is the other half.

The strictness is asserted through the Makefile rather than through the
workflow, because that is where it lives: the job runs ``make lint-clippy``
and the target's flags come from ``CLIPPY_FLAGS``. A contract that pinned
only the step's text would pass while the target it calls had quietly
weakened.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import re
import typing as typ

import pytest
from makefile_support import recipe_lines
from workflow_reading import read_workflows, workflow_jobs, workflow_steps

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".github" / "workflows"
MAKEFILE_PATH: typ.Final[pathlib.Path] = REPOSITORY_ROOT / "Makefile"

#: The job under contract, and the target it must call.
JOB_NAME: typ.Final[str] = "clippy-beta"
LINT_TARGET: typ.Final[str] = "lint-clippy"

#: Both platforms, exactly. Windows is the whole point: it is the only
#: runner that compiles the ``cfg(windows)`` code the 2026-09-28 lints
#: lived in.
REQUIRED_OSES: typ.Final[frozenset[str]] = frozenset(
    {"ubuntu-latest", "windows-latest"}
)

#: The Makefile variable carrying the lint invocation's strictness, and
#: the exact value every caller depends on. ``--workspace`` because a
#: package left out of the invocation is a package nothing lints;
#: ``--all-targets`` because the lints this contract exists for were in
#: integration tests, which a bare ``cargo clippy`` does not build;
#: ``--all-features`` and ``-D warnings`` because the pinned job's
#: reputation is for the strictest reading of the workspace.
CLIPPY_FLAGS_VARIABLE: typ.Final[str] = "CLIPPY_FLAGS"
CLIPPY_FLAGS: typ.Final[str] = "--workspace --all-targets --all-features -- -D warnings"

#: The explicit toolchain selection. ``cargo +beta`` overrides the
#: ``rust-toolchain.toml`` directory pin, without which this job would
#: compile with the pinned toolchain under a beta-sounding name.
BETA_SELECTION: typ.Final[str] = "cargo +beta"

#: The variable the selection travels in.
BETA_VARIABLE: typ.Final[str] = "CARGO"

#: A commit SHA, which is how every action in this repository is pinned.
#: A tag or branch would let a third party change what runs here.
_PINNED_ACTION: typ.Final[re.Pattern[str]] = re.compile(r"@[0-9a-f]{40}$")


@pytest.fixture(scope="module")
def beta_job() -> dict[str, object]:
    """Return the parsed ``clippy-beta`` job.

    Returns
    -------
    dict
        The job's mapping.

    Raises
    ------
    pytest.fail.Exception
        If no workflow declares that job. A missing job would make every
        assertion below iterate nothing.
    """
    jobs = {
        name: job
        for document in read_workflows(WORKFLOWS).values()
        for name, job in workflow_jobs(document).items()
    }
    if JOB_NAME not in jobs:
        pytest.fail(f"no workflow declares a {JOB_NAME!r} job")
    return jobs[JOB_NAME]


@pytest.fixture(scope="module")
def lint_recipe() -> list[str]:
    """Return the ``lint-clippy`` target's recipe lines.

    Returns
    -------
    list of str
        The target's recipe, empty when the target is absent.
    """
    return recipe_lines(MAKEFILE_PATH.read_text(encoding="utf-8"), LINT_TARGET)


def makefile_variable(makefile: str, name: str) -> str | None:
    """Return the default value of a ``?=`` Makefile variable.

    Only the conditional assignment is read. A plain ``=`` for the same
    name would silently override a caller's environment, and a value
    appearing in an ``ifeq`` branch is not the default this contract is
    about.

    Parameters
    ----------
    makefile : str
        The text of a Makefile.
    name : str
        The variable to read.

    Returns
    -------
    str or None
        Everything after the ``?=``, or ``None`` when no such
        assignment exists.

    Examples
    --------
    >>> makefile_variable("V ?= a b c\\n", "V")
    'a b c'
    >>> makefile_variable("V = a\\n", "V") is None
    True
    >>> makefile_variable("W ?= a\\n", "V") is None
    True
    """
    pattern = re.compile(rf"^{re.escape(name)}\s*\?=\s*(.*)$", re.MULTILINE)
    match = pattern.search(makefile)
    return match.group(1).strip() if match else None


def _matrix_oses(job: dict[str, object]) -> frozenset[str]:
    """Return the operating systems one job's matrix names."""
    strategy = job.get("strategy")
    matrix = strategy.get("matrix") if isinstance(strategy, dict) else None
    declared = matrix.get("os") if isinstance(matrix, dict) else None
    return frozenset(str(name) for name in declared or [])


def test_the_lane_covers_both_operating_systems(beta_job: dict[str, object]) -> None:
    """Linux and Windows both run the beta lints.

    Windows is the leg that earns this job its place. Excluding it would
    leave every ``cfg(windows)`` file uncompiled on beta, which is
    exactly the gap the 2026-09-28 failure came through.
    """
    assert _matrix_oses(beta_job) == REQUIRED_OSES, (
        f"{JOB_NAME} must run on {sorted(REQUIRED_OSES)}; it runs on "
        f"{sorted(_matrix_oses(beta_job))}"
    )
    assert beta_job.get("runs-on") == "${{ matrix.os }}", (
        f"{JOB_NAME} must run on the matrix's operating system; it runs on "
        f"{beta_job.get('runs-on')!r}"
    )


def test_a_failing_leg_does_not_cancel_its_sibling(
    beta_job: dict[str, object],
) -> None:
    """``fail-fast: false``, so both legs report.

    With the default, the Linux leg failing would cancel the Windows leg
    mid-flight and the run would show one lint where there might be two.
    """
    strategy = beta_job.get("strategy")
    declared = strategy.get("fail-fast") if isinstance(strategy, dict) else None
    assert declared == "false", (
        f"{JOB_NAME} must declare fail-fast: false; it declares {declared!r}"
    )


def test_the_job_is_advisory_at_job_scope(beta_job: dict[str, object]) -> None:
    """``continue-on-error`` is set, and set on the job rather than a step.

    Job scope is what keeps the lane from blocking a merge while leaving
    each leg's own result visible. A step-scoped setting would do the
    opposite in both directions: it would also swallow a step that
    genuinely succeeded after failing, and it would say nothing about
    the job's aggregate status.
    """
    assert beta_job.get("continue-on-error") == "true", (
        f"{JOB_NAME} must be advisory at job scope; it declares "
        f"continue-on-error: {beta_job.get('continue-on-error')!r}"
    )
    stepped = [
        step.get("name")
        for step in workflow_steps({"jobs": {JOB_NAME: beta_job}})
        if "continue-on-error" in step
    ]
    assert not stepped, (
        f"continue-on-error belongs at job scope, not on {stepped}"
    )


def test_the_job_selects_beta_explicitly(beta_job: dict[str, object]) -> None:
    """The toolchain is chosen by ``+beta``, not by the directory pin.

    ``rust-toolchain.toml`` pins the workspace to stable, and rustup
    honours it for every ``cargo`` invocation that does not name a
    toolchain. Without the override this job would be an expensive
    second opinion on the pinned toolchain.
    """
    selections = [
        step.get("env", {}).get(BETA_VARIABLE)
        for step in workflow_steps({"jobs": {JOB_NAME: beta_job}})
        if isinstance(step.get("env"), dict)
    ]
    assert BETA_SELECTION in selections, (
        f"{JOB_NAME} must pass {BETA_VARIABLE}={BETA_SELECTION!r} to a step; "
        f"it passes {[value for value in selections if value]}"
    )


def test_the_job_installs_beta_with_clippy(beta_job: dict[str, object]) -> None:
    """Beta arrives with Clippy, by component and by pinned action.

    ``components: clippy`` is not implied by the toolchain: the
    ``dtolnay/rust-toolchain`` action installs the component set it is
    given, and a missing one fails the lint step rather than the install
    step, which reads as a lint problem.
    """
    installs = [
        step
        for step in workflow_steps({"jobs": {JOB_NAME: beta_job}})
        if isinstance(step.get("with"), dict)
        and step["with"].get("toolchain") == "beta"
    ]
    assert len(installs) == 1, (
        f"{JOB_NAME} must install the beta toolchain exactly once; it does so "
        f"{len(installs)} times"
    )
    install = installs[0]
    assert install["with"].get("components") == "clippy", (
        f"the beta install must request the clippy component; it requests "
        f"{install['with'].get('components')!r}"
    )
    assert _PINNED_ACTION.search(str(install.get("uses", ""))), (
        f"the toolchain action must be pinned to a commit; it is {install.get('uses')!r}"
    )


def test_the_checkout_is_pinned_and_the_token_is_read_only(
    beta_job: dict[str, object],
) -> None:
    """The job's own action pin and its ``permissions`` block.

    An unpinned action is a third party's ability to change what runs on
    this job's runners, and a job carrying more than ``contents: read``
    would be a token-exercising lane that only lints.
    """
    assert beta_job.get("permissions") == {"contents": "read"}, (
        f"{JOB_NAME} must ask only for contents: read; it asks for "
        f"{beta_job.get('permissions')!r}"
    )
    checkouts = [
        step
        for step in workflow_steps({"jobs": {JOB_NAME: beta_job}})
        if str(step.get("uses", "")).startswith("actions/checkout@")
    ]
    assert checkouts, f"{JOB_NAME} must check the repository out"
    for step in checkouts:
        assert _PINNED_ACTION.search(str(step["uses"])), (
            f"the checkout must be pinned to a commit; it is {step['uses']!r}"
        )


def test_windows_gets_the_tool_the_target_needs(beta_job: dict[str, object]) -> None:
    """``make`` is provisioned on the Windows leg, and only there.

    The toolchain step installs what this job lints *with*, not what it
    lints *through*: ``make`` arrives from the image, and the
    windows-latest image is not this job's to assume. The lint step would
    otherwise fail on a missing command rather than on a lint, which
    reads as a lint failure. The guard keeps the Linux leg from paying
    for the install, and the single-match assertion keeps a second one
    from creeping in beside it.
    """
    provisioning = [
        step
        for step in workflow_steps({"jobs": {JOB_NAME: beta_job}})
        if "make" in str(step.get("run", "")) and "install" in str(step.get("run", ""))
    ]
    assert len(provisioning) == 1, (
        f"{JOB_NAME} must provision make exactly once; it does so "
        f"{len(provisioning)} times"
    )
    step = provisioning[0]
    condition = str(step.get("if", ""))
    assert "windows" in condition, (
        f"provisioning make must be limited to the Windows leg; its condition "
        f"is {condition!r}"
    )


def test_the_job_calls_the_shared_lint_target(beta_job: dict[str, object]) -> None:
    """The lint invocation is the Makefile's, not a copy of it."""
    runs = [
        str(step.get("run", ""))
        for step in workflow_steps({"jobs": {JOB_NAME: beta_job}})
    ]
    assert any(LINT_TARGET in run for run in runs), (
        f"{JOB_NAME} must run make {LINT_TARGET}; it runs {runs}"
    )


def test_the_shared_target_is_strict(lint_recipe: list[str]) -> None:
    """``make lint-clippy`` asks Clippy for the whole workspace, denied.

    Asserted here because the job's strictness is this target's
    behaviour: a workflow contract that read only the step would certify
    a lane whose flags had been relaxed underneath it.
    """
    invocations = [line for line in lint_recipe if "clippy" in line]
    assert invocations, f"{LINT_TARGET} must invoke Clippy; its recipe is {lint_recipe}"
    for line in invocations:
        assert f"$({CLIPPY_FLAGS_VARIABLE})" in line, (
            f"{LINT_TARGET} must take its flags from {CLIPPY_FLAGS_VARIABLE}; "
            f"it runs {line!r}"
        )
    declared = makefile_variable(
        MAKEFILE_PATH.read_text(encoding="utf-8"), CLIPPY_FLAGS_VARIABLE
    )
    assert declared == CLIPPY_FLAGS, (
        f"{CLIPPY_FLAGS_VARIABLE} must default to {CLIPPY_FLAGS!r}; it defaults "
        f"to {declared!r}"
    )
