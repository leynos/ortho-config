"""Contract for provisioning the pinned build tools before any Linux suite runs.

Excluding ``mold`` from a coverage build's flags does not remove the need for
it: the suite that runs under coverage is the suite the Linux ``build-test``
leg runs, and any job that reaches ``make check-build-tools`` fails without the
pinned linker. So every Linux job that runs the suite, through the coverage
action, a direct test invocation or a Make goal that reaches the preflight,
installs the pinned tools first. The installer is held to what makes it a
guarantee rather than a hope:

- it is ``setup-rust`` with ``install-mold: 'true'``, the approved
  digest-verified route, not a script of the job's own;
- it carries no ``if`` and no ``continue-on-error``, so it cannot be skipped or
  its failure swallowed (the action is a notice-only no-op off Linux, which is
  why no platform guard is needed);
- it sits in the same job, ahead of the first step that reaches the suite;
- its ``mold-version`` equals ``tools/mold/VERSION``, the pin the local
  ``make install-build-tools`` and ``make check-build-tools`` share.

The jobs are derived, not listed. ``EXPECTED_SUITE_LEGS`` names the ones this
repository has today only to prove the derivation finds them: a reader that
found none would otherwise pass every assertion over an empty set.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import re
import typing as typ

import pytest
import workflow_legs
from build_standard_make import REPOSITORY_ROOT
from workflow_reading import read_workflows, workflow_jobs

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from workflow_reading import WorkflowDocument

WORKFLOWS: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".github" / "workflows"
MOLD_VERSION_FILE: typ.Final[pathlib.Path] = REPOSITORY_ROOT / "tools" / "mold" / "VERSION"
MOLD_SHA256SUMS: typ.Final[pathlib.Path] = REPOSITORY_ROOT / "tools" / "mold" / "SHA256SUMS"

SETUP_RUST: typ.Final[re.Pattern[str]] = re.compile(
    r"leynos/shared-actions/\.github/actions/setup-rust@[0-9a-f]{40}$"
)

#: The Linux legs that run the suite today. Proof that the derivation works, not
#: the definition of the set: every leg it finds is held to the installer rule.
EXPECTED_SUITE_LEGS: typ.Final[frozenset[str]] = frozenset(
    {"ci.yml:build-test:linux", "coverage-main.yml:coverage-upload"}
)


@pytest.fixture(scope="module")
def documents() -> dict[str, WorkflowDocument]:
    """Return every workflow, parsed once."""
    return read_workflows(WORKFLOWS)


def _all_legs(documents: dict[str, WorkflowDocument]) -> list[workflow_legs.Leg]:
    """Return every leg of every job in every workflow."""
    return [
        leg
        for name, document in sorted(documents.items())
        for job_id, job in workflow_jobs(document).items()
        for leg in workflow_legs.legs(name, job_id, job)
        if "steps" in job
    ]


def _steps(leg: workflow_legs.Leg) -> list[dict[str, object]]:
    """Return the leg's steps that can run on it, in order."""
    steps = leg.job.get("steps", [])
    assert isinstance(steps, list)
    return [step for step in steps if isinstance(step, dict) and workflow_legs.step_runs_on(step, leg.cell)]


def _linux_suite_legs(documents: dict[str, WorkflowDocument]) -> list[workflow_legs.Leg]:
    """Return the Linux legs with at least one step that reaches the suite."""
    return [
        leg
        for leg in _all_legs(documents)
        if workflow_legs.runs_on_linux(leg)
        and any(workflow_legs.reaches_the_suite(step, leg) for step in _steps(leg))
    ]


def _is_installer(step: dict[str, object]) -> bool:
    """Report whether a step is the pinned mold install, however guarded."""
    with_block = step.get("with")
    return (
        SETUP_RUST.search(str(step.get("uses", ""))) is not None
        and isinstance(with_block, dict)
        and with_block.get("install-mold") == "true"
    )


def test_the_derivation_finds_the_known_linux_suite_legs(
    documents: dict[str, WorkflowDocument],
) -> None:
    """An empty or short derivation is a reader fault, not a repository that complies."""
    found = {leg.name for leg in _linux_suite_legs(documents)}
    assert EXPECTED_SUITE_LEGS <= found, (
        f"the derivation missed {sorted(EXPECTED_SUITE_LEGS - found)}; it found {sorted(found)}"
    )


def test_the_derivation_excludes_legs_that_do_not_reach_the_suite(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The beta lane opts out of the standard, so it needs no tools and is not a suite leg.

    Pinned so that excusing a job is a deliberate act: if ``clippy-beta`` loses
    ``BUILD_STANDARD: '0'`` its preflight becomes reachable and it drops out
    of this set into the one that needs an installer.
    """
    found = {leg.name for leg in _linux_suite_legs(documents)}
    assert not any(name.startswith("ci.yml:clippy-beta") for name in found), sorted(found)
    assert not any(name.startswith("ci.yml:msrv") for name in found), sorted(found)


def _suite_leg_cases() -> list[object]:
    """Return one parametrisation per leg the derivation finds, by name."""
    documents = read_workflows(WORKFLOWS)
    return [pytest.param(leg.name, id=leg.name) for leg in _linux_suite_legs(documents)]


@pytest.mark.parametrize("leg_name", _suite_leg_cases())
def test_every_linux_suite_leg_installs_the_pinned_tools_first(
    documents: dict[str, WorkflowDocument], leg_name: str
) -> None:
    """The installer is unconditional, non-soft-failing and ahead of the suite."""
    leg = next(leg for leg in _linux_suite_legs(documents) if leg.name == leg_name)
    steps = _steps(leg)
    first_suite = next(
        index for index, step in enumerate(steps) if workflow_legs.reaches_the_suite(step, leg)
    )
    installers = [index for index, step in enumerate(steps[:first_suite]) if _is_installer(steps[index])]
    assert installers, f"{leg_name} reaches the suite at step {first_suite} with no install-mold before it"
    installer = steps[installers[0]]
    assert "if" not in installer, f"{leg_name}: the installer is conditional: {installer['if']!r}"
    assert "continue-on-error" not in installer, f"{leg_name}: the installer may fail softly"
    assert leg.job.get("continue-on-error") is None, f"{leg_name}: the job swallows failures"
    version = MOLD_VERSION_FILE.read_text(encoding="utf-8").strip()
    declared = installer["with"].get("mold-version")  # type: ignore[union-attr]
    assert declared == version, f"{leg_name}: mold-version is {declared!r}, tools/mold/VERSION is {version!r}"


def test_the_pin_files_agree_with_each_other() -> None:
    """The version file names a release whose archives the checksum file records."""
    version = MOLD_VERSION_FILE.read_text(encoding="utf-8").strip()
    assert re.fullmatch(r"\d+\.\d+\.\d+", version), f"tools/mold/VERSION is {version!r}"
    recorded = {
        line.split()[1]
        for line in MOLD_SHA256SUMS.read_text(encoding="utf-8").splitlines()
        if line.strip()
    }
    expected = {f"mold-{version}-{arch}-linux.tar.gz" for arch in ("x86_64", "aarch64")}
    assert recorded == expected, f"SHA256SUMS records {sorted(recorded)}, not {sorted(expected)}"


def test_the_installer_reader_rejects_the_unsafe_variants() -> None:
    """The three ways to make an installer decorative are each detected.

    The repository's workflows cannot show this, since they use one spelling of
    everything; the helper is exercised on supplied steps instead.
    """
    good = {
        "uses": "leynos/shared-actions/.github/actions/setup-rust@" + "a" * 40,
        "with": {"install-mold": "true"},
    }
    assert _is_installer(good)
    assert not _is_installer({**good, "with": {"install-mold": "false"}})
    assert not _is_installer({**good, "uses": "leynos/shared-actions/.github/actions/setup-rust@v1"})
    assert not _is_installer({"run": "echo install-mold"})
