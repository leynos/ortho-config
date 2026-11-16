"""Contract tests for the production-proof workflow.

The workflow downloads a large verifier binary and runs the repository's real
proofs, so a mistake here is expensive and easy to miss: a missing path filter
means the lane never runs on the change that broke it, an unpinned action runs
unreviewed code, and a swallowed exit status publishes a green job over a red
proof. These tests parse the workflow and pin those properties.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest

from workflow_reading import (
    load_workflow,
    triggers,
    workflow_jobs,
    workflow_steps,
)

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPOSITORY_ROOT / ".github" / "workflows" / "prover-tools.yml"
JOB_NAME = "production-proofs"

#: A full commit SHA. A tag or branch ref would let a repointed action run
#: unreviewed code in a job that executes a downloaded verifier binary.
PINNED_SHA_RE = re.compile(r"^[0-9a-f]{40}$")

#: Inputs whose change must re-run the lane. A missing entry means the lane
#: stays green while the thing it validates has changed.
REQUIRED_PULL_REQUEST_PATHS = (
    ".github/workflows/prover-tools.yml",
    "Makefile",
    "tools/verus/VERSION",
    "tools/verus/SHA256SUMS",
    "verus/**",
    "ortho_config/src/docs/profile_redaction/**",
    "tests/workflow_contracts/prover_tools_test.py",
    "tests/workflow_contracts/prover_tools_workflow_test.py",
)

#: The commands the job must run, in this order. Installing before verifying is
#: the whole point of the lane: execution is forbidden from installing.
ORDERED_COMMANDS = (
    "make install-verus",
    "make verus",
)

#: The transcript path the upload step must publish.
EVIDENCE_DIRECTORY = "target/prover-tools-fixtures/"


def _workflow() -> dict[str, object]:
    """Return the parsed workflow document."""
    return load_workflow(WORKFLOW_PATH.read_text(encoding="utf-8"))


def _job() -> dict[str, object]:
    """Return the single production-proof job, failing when it is absent."""
    jobs = workflow_jobs(_workflow())
    if JOB_NAME not in jobs:
        pytest.fail(f"the workflow declares no {JOB_NAME!r} job")
    return jobs[JOB_NAME]


def _steps() -> list[dict[str, object]]:
    """Return the job's steps in declaration order."""
    return workflow_steps({"jobs": {JOB_NAME: _job()}})


def _run_commands(steps: list[dict[str, object]]) -> list[str]:
    """Return every ``run`` command in order, ignoring other steps."""
    return [
        str(step["run"]).strip()
        for step in steps
        if isinstance(step.get("run"), str)
    ]


def _assert_ordered(steps: list[dict[str, object]]) -> None:
    """Assert the required commands appear once each, in the required order."""
    commands = _run_commands(steps)
    positions = []
    for required in ORDERED_COMMANDS:
        matches = [
            index for index, command in enumerate(commands) if required in command
        ]
        assert len(matches) == 1, f"{required!r} must run exactly once, found {matches}"
        positions.append(matches[0])
    assert positions == sorted(positions), (
        f"commands must run in the order {ORDERED_COMMANDS}, found {commands}"
    )


def test_workflow_runs_for_pull_requests_dispatches_and_schedule() -> None:
    """The lane is reachable from a pull request, a dispatch, and a schedule."""
    declared = triggers(_workflow())
    assert {"pull_request", "workflow_dispatch", "schedule"} <= declared


def test_pull_request_paths_cover_every_proof_input() -> None:
    """A change to any pinned input re-runs the lane."""
    declared = _workflow().get("on", _workflow().get(True))
    filters = declared["pull_request"] if isinstance(declared, dict) else {}
    paths = set(filters.get("paths", []))
    missing = set(REQUIRED_PULL_REQUEST_PATHS) - paths
    assert not missing, f"pull-request paths omit {sorted(missing)}"


def test_job_is_read_only_and_bounded() -> None:
    """The job holds no write scope and cannot outlive its budget."""
    job = _job()
    assert job.get("permissions") == {"contents": "read"}
    # The unique-key reader is a base loader, so every scalar is a string.
    assert job.get("timeout-minutes") == "30"


def test_every_action_is_pinned_to_a_full_commit() -> None:
    """No action reference is a tag or branch."""
    for step in _steps():
        uses = step.get("uses")
        if not isinstance(uses, str):
            continue
        _, _, ref = uses.partition("@")
        assert PINNED_SHA_RE.match(ref), f"{uses!r} is not pinned to a commit"


def test_exactly_one_uv_setup_step() -> None:
    """The job provisions uv once, through the repository's pinned action."""
    uv_steps = [
        step
        for step in _steps()
        if str(step.get("uses", "")).startswith("astral-sh/setup-uv@")
    ]
    assert len(uv_steps) == 1, f"expected one uv setup step, found {len(uv_steps)}"


def test_sccache_is_disabled_for_the_verus_driver() -> None:
    """The job must not hand Verus's compiler driver to sccache.

    Verus drives rustc itself to type-check a proof file, which sccache cannot
    cache. Re-enabling sccache here would wrap a compiler the cache does not
    know and slow the lane down without caching anything.
    """
    setup = [
        step
        for step in _steps()
        if str(step.get("uses", "")).startswith(
            "leynos/shared-actions/.github/actions/setup-rust@"
        )
    ]
    assert len(setup) == 1, "the job must set up Rust exactly once"
    options = setup[0].get("with", {})
    assert options.get("use-sccache") == "false", (
        "Verus's compiler driver must not be wrapped by sccache"
    )


def test_installation_precedes_verification() -> None:
    """Verus is installed before any proof runs."""
    _assert_ordered(_steps())


def test_verification_is_not_masked_by_the_transcript_pipe() -> None:
    """The proof run must fail the step when verification fails.

    ``make verus | tee`` reports ``tee``'s status in a POSIX shell, which would
    publish a green job over a failed proof. The step has to opt into
    ``pipefail`` and the transcript has to be written by that same pipeline.
    """
    proof_steps = [
        step
        for step in _steps()
        if "make verus" in str(step.get("run", ""))
    ]
    assert len(proof_steps) == 1
    script = str(proof_steps[0]["run"])
    assert "set -o pipefail" in script
    assert "tee target/prover-tools-fixtures/verus-production.out" in script


def test_evidence_is_uploaded_even_when_verification_fails() -> None:
    """The verifier transcript survives a failed run."""
    uploads = [
        step
        for step in _steps()
        if str(step.get("uses", "")).startswith("actions/upload-artifact@")
    ]
    assert len(uploads) == 1, "the job must upload exactly one evidence artefact"
    upload = uploads[0]
    assert upload.get("if") == "always()"
    options = upload.get("with", {})
    assert options.get("retention-days") == "14"
    assert options.get("path") == EVIDENCE_DIRECTORY


def test_registered_proofs_are_actually_verified_by_the_lane() -> None:
    """The lane verifies the registered inventory rather than a named subset.

    A lane that ran ``make verus VERUS_FLAGS=...`` for one file would go green
    while a newly registered proof never ran at all.
    """
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    assert "VERUS_PROOF_FILES ?= verus/profile_redaction.rs" in makefile
    for step in _steps():
        command = str(step.get("run", ""))
        assert "VERUS_PROOF_FILES=" not in command
        assert "VERUS_FLAGS=" not in command


def test_workflow_uses_no_secrets() -> None:
    """The lane needs no credential, so none may appear."""
    assert "secrets." not in WORKFLOW_PATH.read_text(encoding="utf-8")


def test_removing_the_verification_step_breaks_the_order_contract() -> None:
    """The order check is not vacuous: dropping the proof run fails it."""
    steps = [
        step
        for step in _steps()
        if "make verus" not in str(step.get("run", ""))
    ]
    with pytest.raises(AssertionError):
        _assert_ordered(steps)