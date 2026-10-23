"""Contract for the Windows command line a coverage build has to fit inside.

Windows passes a spawned process's whole command line as one string, and
``CreateProcess`` refuses one longer than 32,767 characters with
``os error 206`` — before the compiler is ever executed, so nothing in the
Rust tree can observe it and no local gate runs on Windows. Run
39893695039 failed after the first path shortening: the ``ortho_config``
library test's rustc invocation reached 33,497 characters and was refused
730 characters over the limit.

The cost is not spread evenly. Cargo repeats the target root 302 times across
dependency search paths and ``--extern`` arguments. Shortening the effective
target directory is therefore the lever that matters. ``cargo-llvm-cov``
normally appends ``llvm-cov-target`` to ``CARGO_TARGET_DIR``, while trybuild
independently reads ``CARGO_TARGET_DIR`` through Cargo metadata.

This module holds that fix's shape so a later edit cannot quietly restore the
long root, make the drive root an ancestor of source paths during trybuild
normalization, scope the change to a platform that does not need it, hand
Linux an empty value that ``cargo-llvm-cov`` rejects, or pick a root so long
that the leg fails again as the workspace grows.

The arithmetic re-derives the measured invocation with its effective target
directory substituted, so the headroom assertion is a statement about this
workspace's real paths rather than a byte-for-byte replay of a hosted log.

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

#: The effective target directory in the failed invocation.
FAILED_TARGET_DIRECTORY: typ.Final[str] = r"D:\a\_temp\cov\llvm-cov-target"

#: GitHub's substitution for the hosted runner's temporary directory. It is
#: short by design: ``D:\a\_temp`` on the Windows image.
RUNNER_TEMP: typ.Final[str] = "D:\\a\\_temp"

#: The chosen coverage directory is a direct child of the runner's drive.
DRIVE_ROOT: typ.Final[str] = pathlib.PureWindowsPath(RUNNER_TEMP).anchor
COVERAGE_TARGET_DIRECTORY: typ.Final[str] = "llvm-cov-target"
CHOSEN_TARGET_DIRECTORY: typ.Final[str] = str(
    pathlib.PureWindowsPath(DRIVE_ROOT) / COVERAGE_TARGET_DIRECTORY
)

#: The Windows checkout root, which must not contain the coverage target.
CHECKOUT_ROOT: typ.Final[pathlib.PureWindowsPath] = pathlib.PureWindowsPath(
    r"D:\a\ortho-config\ortho-config"
)

#: The measured failed command line and count of its repeated target root.
FAILING_COMMAND_LINE: typ.Final[int] = 33_497
ROOT_OCCURRENCES: typ.Final[int] = 302

#: The workspace package with the longest build-script directory name.
LONGEST_PACKAGE: typ.Final[str] = "orthohelp_policy_adoption_fixture"

#: A representative Cargo build-script unit hash.
UNIT_HASH: typ.Final[str] = "0123456789abcdef"

#: A shorter target directory that still exceeds the limit.
NEAR_MISS_DIRECTORY: typ.Final[str] = r"D:\a\_temp\c\llvm-cov-target"

#: A target directory that fits but leaves less than the required margin.
VERBOSE_DIRECTORY: typ.Final[str] = r"D:\verbose\llvm-cov-target"

#: The selected directory leaves 2,894 characters on the measured invocation.
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


def _chosen_target_directory() -> str:
    """Return the effective coverage directory published on Windows."""
    run = str(_chooser().get("run", ""))
    assert "[System.IO.Path]::GetPathRoot($env:RUNNER_TEMP)" in run, (
        f"the chooser writes {run.strip()!r}, which does not select the "
        f"Windows runner's drive root"
    )
    assert "Join-Path $coverageRoot 'llvm-cov-target'" in run, (
        f"the chooser writes {run.strip()!r}, which does not create the "
        f"single coverage directory under the drive root"
    )
    assert '"CARGO_TARGET_DIR=$coverageTarget"' in run, (
        f"the chooser writes {run.strip()!r}, which does not publish its "
        f"selected root through GITHUB_ENV"
    )
    assert '"CARGO_LLVM_COV_TARGET_DIR=$coverageTarget"' in run, (
        f"the chooser writes {run.strip()!r}, which does not pin the "
        f"cargo-llvm-cov target to the same directory"
    )
    return CHOSEN_TARGET_DIRECTORY


def _projected_command_line(target_directory: str) -> int:
    """Return the command-line length this root would produce.

    Only the part the root influences is re-derived: every occurrence of the
    failed effective target directory is replaced, and the rest of the
    measured invocation is kept. That is honest about what it is — the same
    invocation, moved.
    """
    return FAILING_COMMAND_LINE + ROOT_OCCURRENCES * (
        len(target_directory) - len(FAILED_TARGET_DIRECTORY)
    )


def test_the_coverage_steps_do_not_set_the_root_themselves() -> None:
    """The root arrives from the chooser, so Linux never sees a value.

    Assigning either variable per step needs the Linux cells to resolve to an
    empty string, and cargo-llvm-cov refuses empty target-directory values. It
    also makes the coverage legs read a matrix value the publisher does not,
    which the CV-005 pairing rejects. Both problems disappear when the only
    assignments are in a chooser step that runs on Windows alone.
    """
    for step in _coverage_steps():
        environment = step.get("env") or {}
        for variable in ("CARGO_TARGET_DIR", "CARGO_LLVM_COV_TARGET_DIR"):
            assert variable not in environment, (
                f"{step.get('name')!r} sets {variable} itself; the chooser "
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


def test_the_chosen_directory_is_the_one_the_coverage_steps_read() -> None:
    """The chooser republishes its selected target through the runner environment."""
    run = str(_chooser().get("run", ""))
    assert "GITHUB_ENV" in run, "the chooser must republish through GITHUB_ENV"
    assert _chosen_target_directory() == CHOSEN_TARGET_DIRECTORY


def test_the_chosen_directory_is_outside_source_path_ancestors() -> None:
    """The metadata target must not prefix the checkout's source paths."""
    target_directory = pathlib.PureWindowsPath(_chosen_target_directory())
    assert target_directory.parent == pathlib.PureWindowsPath(DRIVE_ROOT), (
        f"coverage target {target_directory} is not a direct child of "
        f"the Windows drive root"
    )
    assert not CHECKOUT_ROOT.is_relative_to(target_directory), (
        f"coverage target {target_directory} is an ancestor of checkout "
        f"{CHECKOUT_ROOT}"
    )
    assert not target_directory.is_relative_to(CHECKOUT_ROOT), (
        f"coverage target {target_directory} is inside checkout {CHECKOUT_ROOT}"
    )


@pytest.mark.parametrize(
    ("target_directory", "expect_fit"),
    [
        pytest.param(CHOSEN_TARGET_DIRECTORY, True, id="chosen-directory-fits"),
        pytest.param(VERBOSE_DIRECTORY, True, id="verbose-directory-fits"),
        pytest.param(
            NEAR_MISS_DIRECTORY,
            False,
            id="a-shorter-directory-still-overflows",
        ),
        pytest.param(
            FAILED_TARGET_DIRECTORY,
            False,
            id="previous-directory-still-fails",
        ),
    ],
)
def test_the_projection_discriminates(
    target_directory: str, expect_fit: bool
) -> None:
    """The arithmetic accepts the chosen directory and rejects alternatives.

    The near miss is the guard on the guard: a directory shorter than the
    failed one that still overflows must be refused, so the test cannot pass
    by treating any relocation as sufficient.
    """
    length = _projected_command_line(target_directory)
    assert (length <= CREATE_PROCESS_LIMIT) is expect_fit, (
        f"{target_directory!r} projects {length} characters against the "
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
    target_directory = _chosen_target_directory()
    length = _projected_command_line(target_directory)
    verbose = _projected_command_line(VERBOSE_DIRECTORY)
    assert verbose > CREATE_PROCESS_LIMIT - MINIMUM_HEADROOM, (
        f"{VERBOSE_DIRECTORY!r} projects {verbose} characters and clears the "
        f"headroom floor, so this bound no longer distinguishes a terse root "
        f"from a merely shorter one"
    )
    assert length <= CREATE_PROCESS_LIMIT - MINIMUM_HEADROOM, (
        f"the target directory {target_directory!r} "
        f"({len(target_directory)} characters, paid "
        f"{ROOT_OCCURRENCES} times) projects {length} characters against the "
        f"{CREATE_PROCESS_LIMIT}-character limit, leaving less than "
        f"{MINIMUM_HEADROOM} characters of headroom"
    )


def test_the_longest_crate_name_still_fits_per_unit() -> None:
    """The per-unit suffix is not a length risk at this target directory.

    Cargo builds the path from the package name, and the workspace's longest
    package is known, so this pins the piece the headroom assertion does not
    model explicitly.
    """
    unit = (
        f"{_chosen_target_directory()}\\debug\\build\\{LONGEST_PACKAGE}\\{UNIT_HASH}\\out"
    )
    assert len(unit) < 200, (
        f"one build-script unit path is {len(unit)} characters; a root this "
        f"deep leaves too little for the rest of the command line"
    )
