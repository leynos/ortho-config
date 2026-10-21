"""Contract for the Windows command line a coverage build has to fit inside.

Windows passes a spawned process's whole command line as one string, and
``CreateProcess`` refuses one longer than 32,767 characters with
``os error 206`` — before the compiler is ever executed, so nothing in the
Rust tree can observe it and no local gate runs on Windows. Run
39645228205 failed exactly there: the ``cargo-orthohelp`` bin test's rustc
invocation reached 33,406 characters and was refused 639 characters over the
limit.

The cost is not spread evenly. Cargo passes one
``-L dependency=<target>/llvm-cov-target/debug/build/<crate>/<hash>/out``
argument per build-script unit (207 in that invocation) and one ``--extern``
path per dependency (43), so the single 53-character checkout root appears
251 times and accounts for 13,303 characters on its own. Shortening the root
is therefore the lever that matters, and ``cargo-llvm-cov`` takes its tree
from ``CARGO_TARGET_DIR``.

This module holds that fix's shape so a later edit cannot quietly restore the
long root, scope it to a platform that does not need it, hand Linux an empty
value that ``cargo-llvm-cov`` rejects, or pick a root so long that the leg
fails again on the next dependency to arrive.

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

#: The root the failing invocation used: the checked-out workspace's
#: ``target``, plus the ``llvm-cov-target`` subdirectory cargo-llvm-cov
#: appends to whatever root it is given.
CHECKOUT_ROOT: typ.Final[str] = r"D:\a\ortho-config\ortho-config\target\llvm-cov-target"

#: GitHub's substitution for the hosted runner's temporary directory. It is
#: short by design: ``D:\a\_temp`` on the Windows image.
RUNNER_TEMP: typ.Final[str] = "D:\\a\\_temp"

#: The directory name the coverage steps use under ``RUNNER_TEMP``. It is
#: deliberately terse: every character here is paid 251 times.
COVERAGE_DIRECTORY: typ.Final[str] = "cov"

#: The longest crate name in the workspace. Cargo's per-unit directory uses
#: the package name, so this sets the worst-case per-unit length.
LONGEST_PACKAGE: typ.Final[str] = "orthohelp_policy_adoption_fixture"

#: Per-unit suffix under the target root: ``debug/build/<crate>/<hash>/out``.
#: Both hex components in the failing invocation were 16 digits.
UNIT_HASH: typ.Final[str] = "0123456789abcdef"

#: The measured failing command line, and how many times the checkout root
#: occurred in it: 207 ``-L dependency`` arguments and 43 ``--extern`` paths.
FAILING_COMMAND_LINE: typ.Final[int] = 33_406
ROOT_OCCURRENCES: typ.Final[int] = 251

#: A root shorter than the checkout's that still exceeds the limit. Trimming
#: the checkout root by three characters removes 753 of the overrun and
#: leaves the rest, which proves the assertion responds to the arithmetic
#: rather than to "the root moved".
NEAR_MISS_ROOT: typ.Final[str] = r"D:\a\ortho-config\ortho-config\target\llvm-cov-targe"

#: A root that clears the limit but not the margin below. ``cargo-target``
#: was the first candidate and leaves under 2,900 characters.
VERBOSE_ROOT: typ.Final[str] = r"D:\a\_temp\cargo-target\llvm-cov-target"

#: The minimum margin the Windows leg must keep. It still carries the sccache
#: and rustc paths, 52 lint flags and the ``-L native`` arguments, and the
#: next dependency widens the command line again.
MINIMUM_HEADROOM: typ.Final[int] = 4_000

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
    prefix = "CARGO_TARGET_DIR=$env:RUNNER_TEMP\\"
    start = run.find(prefix)
    assert start != -1, (
        f"the chooser writes {run.strip()!r}, which does not publish a "
        f"CARGO_TARGET_DIR under RUNNER_TEMP"
    )
    remainder = run[start + len(prefix) :]
    return RUNNER_TEMP + "\\" + remainder.split('"')[0].strip()


def _projected_command_line(root: str) -> int:
    """Return the command-line length this root would produce.

    Only the part the root influences is re-derived: every occurrence of the
    checkout root is replaced, and the rest of the measured invocation is
    kept. That is honest about what it is — the same invocation, moved.
    """
    return FAILING_COMMAND_LINE + ROOT_OCCURRENCES * (
        len(root) - len(CHECKOUT_ROOT)
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
    assert f"CARGO_TARGET_DIR=$env:RUNNER_TEMP\\{COVERAGE_DIRECTORY}" in run, (
        f"the chooser writes {run.strip()!r}, which is not the runner-temp "
        f"root the coverage steps read"
    )


@pytest.mark.parametrize(
    ("root", "expect_fit"),
    [
        pytest.param(f"{RUNNER_TEMP}\\{COVERAGE_DIRECTORY}\\llvm-cov-target",
                     True, id="chosen-root-fits"),
        pytest.param(NEAR_MISS_ROOT, False, id="a-shorter-root-still-overflows"),
        pytest.param(CHECKOUT_ROOT, False, id="checkout-root-still-fails"),
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
    would fail again on the next dependency, so the bound asserts the room
    the Windows leg still has for its compiler paths, ``--extern`` list and
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
