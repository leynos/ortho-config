"""Contract for the builds the development standard must stay out of, and the MSRV lane.

Three kinds of build are excluded from the standard, and each is excluded by
the same mechanism, which is why it is worth proving rather than assuming:
Cargo takes ``RUSTFLAGS`` in preference to every ``rustflags`` source in
``.cargo/config.toml``, so assigning it, even to a value that does nothing,
displaces the standard.

- Coverage appends ``-Cinstrument-coverage`` to ``RUSTFLAGS`` and is measured
  with the platform linker. Instrumentation is LLVM-only, so the explicit
  assignment must keep the selected development backend out of coverage.
- Release and packaging builds are production builds: LLVM, the platform
  linker, the stable frontend settings.
- The MSRV lane runs a stable compiler, which refuses the standard's
  nightly-only flags outright.

The failure mode is the same in each: ``RUSTFLAGS`` is unset, the
configuration applies, and a coverage run is linked by the wrong linker (or a
release ships a development-flag binary) without anyone having edited the build. So the contract is
that each such job sets a ``RUSTFLAGS`` with none of the standard in it (the
frontend, the linker, or a backend), and that nothing in the job empties it
again: an empty ``RUSTFLAGS`` input to the
setup action leaves the variable unset, which is the same as leaving the
configuration in force.

The MSRV lane builds every target on the MSRV toolchain and never runs a test
(the estate's cost rule is a ``cargo check``; a build is stronger and was asked
for), on the version the manifest
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


def _declared_msrv_leg() -> str:
    """Return the declared ``rust-version`` as the ``major.minor`` a matrix leg names."""
    return ".".join(_declared_msrv().split(".")[:2])


#: Commands that would turn the MSRV lane into a second test run.
TEST_COMMANDS: typ.Final[frozenset[str]] = frozenset({"test", "nextest", "llvm-cov", "mutants"})


def _msrv_job(documents: dict[str, WorkflowDocument]) -> dict[str, typ.Any]:
    """Return the ``msrv`` job."""
    return typ.cast("dict[str, typ.Any]", _job(documents, "ci.yml", "msrv"))


def test_the_msrv_matrix_leg_is_the_workspace_rust_version(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The matrix is keyed by toolchain and has exactly one leg: the declared MSRV.

    The leg is compared with the manifest's ``major.minor``, so raising
    ``rust-version`` without moving the leg (or the reverse) fails here and the
    two cannot drift. ``major.minor`` is also what the estate's MSRV audit
    matches a workflow's numeric toolchain against.
    """
    matrix = _msrv_job(documents)["strategy"]["matrix"]
    assert matrix["toolchain"] == [_declared_msrv_leg()], (
        f"the MSRV matrix is {matrix.get('toolchain')!r}; the manifest declares {_declared_msrv()!r}"
    )


def test_the_msrv_leg_installs_and_builds_its_matrix_toolchain_and_runs_no_tests(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The leg installs ``matrix.toolchain``, builds through Make, and never tests.

    A build is stronger than the estate's ``cargo check`` cost rule (it links)
    and is still not a test run; the maintainer asked for it. What the lane may
    not become is a second execution of the suite.
    """
    job = _msrv_job(documents)
    steps = [step for step in job["steps"] if isinstance(step, dict)]
    installs = [_mapping(step.get("with")).get("toolchain") for step in steps if "with" in step]
    assert installs == ["${{ matrix.toolchain }}"], f"the MSRV leg installs {installs}"
    runs = [str(step["run"]) for step in steps if "run" in step]
    assert runs == ["make msrv-build MSRV=${{ matrix.toolchain }}"], f"the MSRV leg runs {runs}"
    for run in runs:
        assert not TEST_COMMANDS & set(run.split()), f"the MSRV leg runs tests: {run!r}"
    for step in steps:
        if "uses" in step:
            assert _PINNED.search(str(step["uses"])), f"{step['uses']!r} is not pinned to a commit"


def test_the_msrv_leg_does_not_inherit_the_nightly_flags(
    documents: dict[str, WorkflowDocument],
) -> None:
    """Nothing in the leg's environment puts the standard back in front of a stable compiler.

    ``make msrv-build`` assigns ``RUSTFLAGS`` itself, but a job- or step-level
    value naming a nightly flag would be a second, contradictory statement.
    """
    job = _msrv_job(documents)
    envs = [_mapping(job.get("env"))] + [
        _mapping(step.get("env")) for step in job["steps"] if isinstance(step, dict)
    ]
    for env in envs:
        flags = set(env.get("RUSTFLAGS", "").split())
        assert not (STANDARD_FLAGS & flags), f"the MSRV leg sets {sorted(STANDARD_FLAGS & flags)}"


def test_the_msrv_version_is_quoted_in_the_workflow_text() -> None:
    """YAML reads an unquoted ``1.80`` as the number 1.8, so the leg must be a string.

    The workflow reader keeps every scalar as written and cannot see the
    difference; the file text can.
    """
    text = (WORKFLOWS / "ci.yml").read_text(encoding="utf-8")
    assert re.search(rf'toolchain:\s*\[\s*["\']{re.escape(_declared_msrv_leg())}["\']\s*\]', text), (
        f"ci.yml does not list the MSRV {_declared_msrv_leg()} as a quoted string"
    )
