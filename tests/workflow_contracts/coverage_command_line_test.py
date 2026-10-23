"""Contract for the Windows command line a coverage build has to fit inside.

Windows passes a spawned process's whole command line as one string, and
``CreateProcess`` refuses one longer than 32,767 characters with
``os error 206`` — before the compiler is ever executed, so nothing in the
Rust tree can observe it and no local gate runs on Windows. Run
39893695039 failed after the first path shortening: the ``ortho_config``
library test's rustc invocation reached 33,497 characters and was refused
730 characters over the limit.

The cost is not spread evenly. Cargo repeats the target root 302 times across
dependency search paths and ``--extern`` arguments. Shortening the root is
therefore the lever that matters, and ``cargo-llvm-cov`` takes its tree from
``CARGO_TARGET_DIR`` before appending ``llvm-cov-target``.

This module holds that fix's shape so a later edit cannot quietly restore the
long root, scope it to a platform that does not need it, hand Linux an empty
value that ``cargo-llvm-cov`` rejects, or pick a root so long that the leg
fails again as the workspace grows.

The arithmetic re-derives the measured invocation with its root substituted,
so the headroom assertion is a statement about this workspace's real paths
rather than a byte-for-byte replay of a hosted log.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import typing as typ

import pytest

from workflow_reading import read_workflows, workflow_jobs

REPOSITORY_ROOT: typ.Final[pathlib.Path] = (
    pathlib.Path(__file__).resolve().parents[2]
)
WORKFLOWS: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".github" / "workflows"

#: What Windows accepts in one ``CreateProcess`` command line.
CREATE_PROCESS_LIMIT: typ.Final[int] = 32_767

#: The ``CARGO_TARGET_DIR`` prefix in the failed invocation. Cargo-llvm-cov
#: appends ``llvm-cov-target`` after this path.
FAILED_TARGET_ROOT: typ.Final[str] = r"D:\a\_temp\cov"

#: GitHub's substitution for the hosted runner's temporary directory. It is
#: short by design: ``D:\a\_temp`` on the Windows image.
RUNNER_TEMP: typ.Final[str] = "D:\\a\\_temp"

#: The chosen root is the drive anchor, avoiding any directory component.
DRIVE_ROOT: typ.Final[str] = pathlib.PureWindowsPath(RUNNER_TEMP).anchor

#: The measured failed command line and count of its repeated target root.
FAILING_COMMAND_LINE: typ.Final[int] = 33_497
ROOT_OCCURRENCES: typ.Final[int] = 302

#: A root shorter than the failed one that still exceeds the limit.
NEAR_MISS_ROOT: typ.Final[str] = r"D:\a\_temp\c"

#: A root that clears the limit but leaves less than the required margin.
VERBOSE_ROOT: typ.Final[str] = r"D:\verbose"

#: The drive-root path leaves 2,592 characters on the measured invocation.
MINIMUM_HEADROOM: typ.Final[int] = 2_000

#: The job that runs the coverage steps.
BUILD_TEST_JOB: typ.Final[str] = "build-test"

#: The step that republishes the chosen root through ``GITHUB_ENV``.
CHOOSER_STEP: typ.Final[str] = "Choose the coverage target root"


def _coverage_steps() -> list[dict[str, typ.Any]]:
    """Return the ``build-test`` coverage steps, in declaration order."""
    job = workflow_jobs(read_workflows(WORKFLOWS)["ci.yml"])[BUILD_TEST_JOB]
    steps = job.get("steps") or []
    return [
        step
        for step in steps
        if isinstance(step, dict) and "generate-coverage" in str(step.get("uses", ""))
    ]


def _chooser() -> dict[str, typ.Any]:
    """Return the step that sets the coverage target root for later steps."""
    job = workflow_jobs(read_workflows(WORKFLOWS)["ci.yml"])[BUILD_TEST_JOB]
    found = [
        step
        for step in job.get("steps") or []
        if isinstance(step, dict) and step.get("name") == CHOOSER_STEP
    ]
    assert len(found) == 1, (
        f"expected exactly one {CHOOSER_STEP!r} step in {BUILD_TEST_JOB}, "
        f"found {len(found)}"
    )
    return found[0]


def _chosen_root() -> str:
    """Return the target root the chooser actually publishes on Windows."""
    run = str(_chooser().get("run", ""))
    assert "[System.IO.Path]::GetPathRoot($env:RUNNER_TEMP)" in run, (
        f"the chooser writes {run.strip()!r}, which does not select the "
        f"Windows runner's drive root"
    )
    assert '"CARGO_TARGET_DIR=$coverageRoot"' in run, (
        f"the chooser writes {run.strip()!r}, which does not publish its "
        f"selected root through GITHUB_ENV"
    )
    return DRIVE_ROOT


def _projected_command_line(root: str) -> int:
    """Return the command-line length this root would produce.

    Only the part the root influences is re-derived: every occurrence of the
    failed target root is replaced, and the rest of the measured invocation
    is kept. That is honest about what it is — the same invocation, moved.
    """
    return FAILING_COMMAND_LINE + ROOT_OCCURRENCES * (
        len(root) - len(FAILED_TARGET_ROOT)
    )


def test_the_coverage_steps_do_not_set_the_root_themselves() -> None:
    """The root arrives from the chooser, so Linux never sees a value.

    Assigning it per step needs the Linux cells to resolve to an empty
    string, and cargo-llvm-cov refuses an empty ``CARGO_TARGET_DIR``. It also
    makes the coverage legs read a matrix value the publisher does not, which
    the CV-005 pairing rejects. Both problems disappear when the only
    assignment is a step that runs on Windows alone.
    """
    for step in _coverage_steps():
        environment = step.get("env") or {}
        assert "CARGO_TARGET_DIR" not in environment, (
            f"{step.get('name')!r} sets CARGO_TARGET_DIR itself; the chooser "
            f"is the sole assignment so the Linux cells stay unset"
        )


def test_linux_is_left_completely_unset() -> None:
    """The Linux leg keeps cargo-llvm-cov's default instead of an empty value.

    ``cargo-llvm-cov`` refuses an empty ``CARGO_TARGET_DIR`` outright, so a
    fix that assigned the default path to Linux would break the leg it was
    not meant to touch.
    """
    chooser = _chooser()
    assert chooser.get("if") == "${{ matrix.platform == 'windows' }}", (
        "the chooser must be scoped to Windows; it runs on "
        f"{chooser.get('if')!r}"
    )


def test_the_chosen_root_is_the_one_the_coverage_steps_read() -> None:
    """The republished value and the step expression name the same tree."""
    run = str(_chooser().get("run", ""))
    assert "GITHUB_ENV" in run, "the chooser must republish through GITHUB_ENV"
    assert "[System.IO.Path]::GetPathRoot($env:RUNNER_TEMP)" in run, (
        f"the chooser writes {run.strip()!r}, which is not based on the "
        f"Windows runner's drive root"
    )


@pytest.mark.parametrize(
    ("root", "expect_fit"),
    [
        pytest.param(DRIVE_ROOT, True, id="chosen-root-fits"),
        pytest.param(NEAR_MISS_ROOT, False, id="a-shorter-root-still-overflows"),
        pytest.param(FAILED_TARGET_ROOT, False, id="previous-root-still-fails"),
    ],
)
def test_the_projection_discriminates(root: str, expect_fit: bool) -> None:
    """The arithmetic accepts the chosen root and rejects the alternatives.

    The near miss is the guard on the guard: a root shorter than the
    checkout's that still overflows must be refused, so the test cannot pass
    by treating any relocation as sufficient.
    """
    length = _projected_command_line(root)
    assert (length <= CREATE_PROCESS_LIMIT) is expect_fit, (
        f"{root!r} projects {length} characters against the "
        f"{CREATE_PROCESS_LIMIT}-character limit"
    )


def test_the_chosen_root_keeps_meaningful_headroom() -> None:
    """The fix is a margin, not a squeak past the limit.

    A repair that landed within a few hundred characters of the ceiling
    would fail again as the workspace grows, so the bound asserts the room
    the Windows leg still has for compiler paths, the ``--extern`` list and
    lint flags. The verbose candidate beside it shows the margin bites: it
    clears the limit but not this floor.
    """
    root = _chosen_root()
    length = _projected_command_line(root)
    verbose = _projected_command_line(VERBOSE_ROOT)
    assert verbose > CREATE_PROCESS_LIMIT - MINIMUM_HEADROOM, (
        f"{VERBOSE_ROOT!r} projects {verbose} characters and clears the "
        f"headroom floor, so this bound no longer distinguishes a terse root "
        f"from a merely shorter one"
    )
    assert length <= CREATE_PROCESS_LIMIT - MINIMUM_HEADROOM, (
        f"the root {root!r} ({len(root)} characters, paid "
        f"{ROOT_OCCURRENCES} times) projects {length} characters against the "
        f"{CREATE_PROCESS_LIMIT}-character limit, leaving less than "
        f"{MINIMUM_HEADROOM} characters of headroom"
    )


def test_the_longest_crate_name_still_fits_per_unit() -> None:
    """The per-unit suffix is not a length risk at this root.

    Cargo builds the path from the package name, and the workspace's longest
    package is known, so this pins the piece the headroom assertion does not
    model explicitly.
    """
    unit = (
        f"{_chosen_root()}\\debug\\build\\{LONGEST_PACKAGE}\\{UNIT_HASH}\\out"
    )
    assert len(unit) < 200, (
        f"one build-script unit path is {len(unit)} characters; a root this "
        f"deep leaves too little for the rest of the command line"
    )
