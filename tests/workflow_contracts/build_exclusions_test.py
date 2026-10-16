"""Contract for the builds the development standard must stay out of, and the MSRV lane.

Three kinds of build are excluded from the standard, and each is excluded by
the same mechanism, which is why it is worth proving rather than assuming:
Cargo takes ``RUSTFLAGS`` in preference to every ``rustflags`` source in
``.cargo/config.toml``, so assigning it, even to a value that does nothing,
displaces the standard.

- Coverage appends ``-Cinstrument-coverage`` to ``RUSTFLAGS`` and is measured
  with the platform linker. Instrumentation is LLVM-only, so a codegen backend
  selected through ``rustflags`` would break it outright; none is selected
  today (``build_standard_routing_test.py`` refuses one), but the exclusion
  must hold for the day one is adopted.
- Release and packaging builds are production builds: LLVM, the platform
  linker, the stable frontend settings.
- The MSRV lane runs a stable compiler, which refuses the standard's
  nightly-only flags outright.

The failure mode is the same in each: ``RUSTFLAGS`` is unset, the
configuration applies, and a coverage run fails (or a release ships a
Cranelift binary) without anyone having edited the build. So the contract is
that each such job sets a ``RUSTFLAGS`` with none of the standard in it (the
frontend, the linker, or a backend), and that nothing in the job empties it
again: an empty ``RUSTFLAGS`` input to the
setup action leaves the variable unset, which is the same as leaving the
configuration in force.

The MSRV lane is held to the estate's cost rule: ``cargo check --all-targets``
on the MSRV toolchain, never a second test run, on the version the manifest
declares.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import re
import tomllib
import typing as typ

import pytest
from build_standard_make import REPOSITORY_ROOT, STANDARD_FLAGS
from workflow_reading import read_workflows, workflow_jobs

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from workflow_reading import WorkflowDocument

WORKFLOWS: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".github" / "workflows"

#: Jobs that must take the production or coverage route, by workflow and job.
EXCLUDED_JOBS: typ.Final[list[tuple[str, str, str]]] = [
    ("coverage_build_test", "ci.yml", "build-test"),
    ("coverage_main", "coverage-main.yml", "coverage-upload"),
    ("packaging_dry_run", "ci.yml", "binstall-packaging"),
    ("release_build", "release.yml", "build-assets"),
]

_PINNED: typ.Final[re.Pattern[str]] = re.compile(r"@[0-9a-f]{40}$")


@pytest.fixture(scope="module")
def documents() -> dict[str, WorkflowDocument]:
    """Return every workflow, parsed once."""
    return read_workflows(WORKFLOWS)


def _job(documents: dict[str, WorkflowDocument], workflow: str, job_id: str) -> dict[str, object]:
    """Return one job, failing clearly if it is gone."""
    jobs = workflow_jobs(documents[workflow])
    assert job_id in jobs, f"{workflow} no longer has a {job_id!r} job: {sorted(jobs)}"
    return jobs[job_id]


def _mapping(value: object) -> dict[str, str]:
    """Return *value* as a string mapping, empty when it is not one."""
    return {str(k): str(v) for k, v in value.items()} if isinstance(value, dict) else {}


@pytest.mark.parametrize(
    ("workflow", "job_id"),
    [pytest.param(w, j, id=case) for case, w, j in EXCLUDED_JOBS],
)
def test_excluded_jobs_assign_rustflags_without_the_standard(
    documents: dict[str, WorkflowDocument], workflow: str, job_id: str
) -> None:
    """The job's ``RUSTFLAGS`` is set, non-empty, and free of the standard."""
    effective = {**_mapping(documents[workflow].get("env")), **_mapping(_job(documents, workflow, job_id).get("env"))}
    assert "RUSTFLAGS" in effective, f"{workflow}:{job_id} leaves RUSTFLAGS unset, so the configuration applies"
    flags = effective["RUSTFLAGS"].split()
    assert flags, f"{workflow}:{job_id} sets RUSTFLAGS to an empty value"
    assert not (STANDARD_FLAGS & set(flags)), (
        f"{workflow}:{job_id} carries the development standard: {sorted(STANDARD_FLAGS & set(flags))}"
    )


@pytest.mark.parametrize(
    ("workflow", "job_id"),
    [pytest.param(w, j, id=case) for case, w, j in EXCLUDED_JOBS],
)
def test_nothing_in_an_excluded_job_empties_or_rewrites_rustflags(
    documents: dict[str, WorkflowDocument], workflow: str, job_id: str
) -> None:
    """No step re-exports ``RUSTFLAGS``, sets it empty, or hands the action an empty one."""
    for step in _job(documents, workflow, job_id).get("steps", []):  # type: ignore[union-attr]
        assert isinstance(step, dict)
        name = step.get("name", step.get("uses", step.get("run")))
        assert "RUSTFLAGS" not in _mapping(step.get("env")), f"{workflow}:{job_id} step {name!r} sets RUSTFLAGS"
        assert "RUSTFLAGS" not in str(step.get("run", "")), f"{workflow}:{job_id} step {name!r} rewrites RUSTFLAGS"
        assert _mapping(step.get("with")).get("rustflags", "x") != "", (
            f"{workflow}:{job_id} step {name!r} passes an empty rustflags input, which leaves the configuration in force"
        )


# -- the MSRV lane -------------------------------------------------------------------


def _declared_msrv() -> str:
    """Return the workspace's declared ``rust-version``."""
    manifest = tomllib.loads((REPOSITORY_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return str(manifest["workspace"]["package"]["rust-version"])


def test_the_msrv_job_checks_the_declared_version_and_runs_no_tests(
    documents: dict[str, WorkflowDocument],
) -> None:
    """One job, named for the MSRV, installing the manifest's version and only checking."""
    job = _job(documents, "ci.yml", "msrv")
    steps = [step for step in job["steps"] if isinstance(step, dict)]  # type: ignore[union-attr]
    toolchains = [_mapping(step.get("with")).get("toolchain") for step in steps if "with" in step]
    assert _declared_msrv() in toolchains, (
        f"the MSRV job installs {toolchains}, the manifest declares {_declared_msrv()!r}"
    )
    runs = [str(step["run"]) for step in steps if "run" in step]
    assert runs == ["make msrv-check"], f"the MSRV job runs {runs}, not the one Make target"
    for step in steps:
        if "uses" in step:
            assert _PINNED.search(str(step["uses"])), f"{step['uses']!r} is not pinned to a commit"


def test_the_msrv_version_is_quoted_in_the_workflow_text() -> None:
    """YAML reads an unquoted ``1.80`` as the number 1.8, so the version must be a string.

    The workflow reader keeps every scalar as written and cannot see the
    difference; the file text can.
    """
    text = (WORKFLOWS / "ci.yml").read_text(encoding="utf-8")
    assert re.search(rf'toolchain:\s*["\']{re.escape(_declared_msrv())}["\']', text), (
        f"ci.yml does not install the MSRV {_declared_msrv()} as a quoted string"
    )
