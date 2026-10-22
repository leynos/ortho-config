"""Contract for the ``docstring-coverage`` gate and its file discovery.

``make lint`` runs ``interrogate`` over every tracked and non-ignored untracked
Python file. Reading the Makefile cannot confirm that promise: the discovered
paths could arrive split into words, a failing discovery could be swallowed by
the pipeline's exit status, and the documented exclusions could fail to hold.

Each test runs the repository's own Makefile against a throwaway Git repository
with ``make -C <fixture> -f <repository>/Makefile``, so the recipe's discovery
and shell expansion are the things under test. ``UV_RUN`` is replaced by a stub
that records its arguments, which avoids needing ``uv`` or ``interrogate``
while still proving exactly what the gate hands over. ``GIT`` and ``MAKE``
are replaced where a test needs to make one of them fail.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import shutil
import stat
import subprocess  # noqa: S404 - the contract runs make and git directly
import typing as typ
from pathlib import Path

import pytest

REPOSITORY_ROOT: typ.Final[Path] = Path(__file__).resolve().parents[2]
MAKEFILE: typ.Final[Path] = REPOSITORY_ROOT / "Makefile"

#: The gate target and the umbrella target that must depend on it.
GATE_TARGET: typ.Final[str] = "docstring-coverage"
LINT_TARGET: typ.Final[str] = "lint"

#: The fixed arguments the gate passes before the inventory. The trailing `--`
#: keeps a leading `-` in a filename from being read as an option.
GATE_ARGUMENTS: typ.Final[tuple[str, ...]] = (
    "interrogate",
    "--fail-under",
    "100",
    "--ext",
    "pyi",
    "--",
)

#: Tracked paths whose shapes a whitespace split or unquoted expansion corrupts.
TRACKED_AWKWARD: typ.Final[tuple[str, ...]] = (
    "scripts/local helper.py",
    "scripts/plain.py",
)

#: Untracked-but-not-ignored paths with the same awkward shapes, plus a stub.
UNTRACKED_AWKWARD: typ.Final[tuple[str, ...]] = (
    "scripts/quote'd.py",
    "scripts/nested/tab\tname.py",
    "scripts/stub.pyi",
)

#: A tracked file inside a hidden directory, which discovery must not skip.
HIDDEN_DIRECTORY_FILE: typ.Final[str] = ".github/actions/probe.py"

#: A tracked file with no docstring, the shape the gate exists to reject.
UNDOCUMENTED_FILE: typ.Final[str] = "scripts/undocumented.py"

#: A tracked file the extension filter must leave out.
TRACKED_NON_PYTHON: typ.Final[str] = "scripts/notes.txt"

#: An untracked file the ignore rule must leave out.
IGNORED_PYTHON: typ.Final[str] = "scripts/ignored.py"

ALL_SCANNED: typ.Final[tuple[str, ...]] = (
    TRACKED_AWKWARD
    + UNTRACKED_AWKWARD
    + (HIDDEN_DIRECTORY_FILE, UNDOCUMENTED_FILE)
)


def _git() -> str:
    """Return the Git executable, failing the test when it is absent."""
    git = shutil.which("git")
    assert git is not None, "git must be available to run the contract"
    return git


def _run_git(directory: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
    """Run one Git command in ``directory``, failing the test on error."""
    return subprocess.run(  # noqa: S603 - fixed argument vector, no shell
        [_git(), *arguments],
        cwd=directory,
        check=True,
        capture_output=True,
        text=True,
    )


def _write(directory: Path, relative: str, content: str) -> None:
    """Create one fixture file, with its parent directories."""
    path = directory / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


DOCUMENTED: typ.Final[str] = '"""Fixture module."""\n\n\ndef probe() -> int:\n    """Return a documented value."""\n    return 1\n'

UNDOCUMENTED: typ.Final[str] = "def probe() -> int:\n    return 1\n"


def _build_fixture_repository(directory: Path) -> None:
    """Create a Git repository whose Python inventory has awkward names.

    Tracked files exercise the ``--cached`` half of the discovery and the
    untracked ones exercise ``--others``; the ignored and non-Python files
    exercise the two documented exclusions. Every scanned file is fully
    documented except ``UNDOCUMENTED_FILE``, so a real ``interrogate`` run
    over this fixture is expected to fail.
    """
    _run_git(directory, "init", "--quiet", "--initial-branch", "main")
    _run_git(directory, "config", "user.email", "contract@example.invalid")
    _run_git(directory, "config", "user.name", "Contract")
    (directory / ".gitignore").write_text(f"{IGNORED_PYTHON}\n", encoding="utf-8")
    for relative in TRACKED_AWKWARD + (TRACKED_NON_PYTHON, HIDDEN_DIRECTORY_FILE):
        _write(directory, relative, DOCUMENTED)
    _write(directory, UNDOCUMENTED_FILE, UNDOCUMENTED)
    _run_git(directory, "add", ".")
    _run_git(directory, "commit", "--quiet", "-m", "fixture")
    for relative in UNTRACKED_AWKWARD + (IGNORED_PYTHON,):
        _write(directory, relative, DOCUMENTED)


def _fake_runner(directory: Path, exit_status: int) -> tuple[Path, Path]:
    """Write a runner stub that records each argument on its own line.

    Parameters
    ----------
    directory : Path
        Where the stub and its argument log are created.
    exit_status : int
        The status the stub exits with.

    Returns
    -------
    tuple of Path
        The stub's path and the file its arguments are appended to.
    """
    log = directory / "arguments.log"
    stub = directory / "fake-uv-run"
    stub.write_text(
        f'#!/bin/sh\nprintf "%s\\n" "$@" >> "{log}"\nexit {exit_status}\n',
        encoding="utf-8",
    )
    stub.chmod(stub.stat().st_mode | stat.S_IXUSR)
    return stub, log


def _run_make(
    fixture: Path, *overrides: str
) -> subprocess.CompletedProcess[str]:
    """Run the gate from ``fixture`` using the repository's Makefile.

    Parameters
    ----------
    fixture : Path
        The throwaway repository the recipe runs in.
    *overrides : str
        ``NAME=value`` assignments, used to replace ``UV_RUN`` or
        ``PYTHON_DEPS_FILE`` for the run.

    Returns
    -------
    subprocess.CompletedProcess of str
        The finished process, with its output captured.
    """
    make = shutil.which("make")
    assert make is not None, "make must be available to run the contract"
    return subprocess.run(  # noqa: S603 - fixed argument vector, no shell
        [make, "-C", str(fixture), "-f", str(MAKEFILE), GATE_TARGET, *overrides],
        capture_output=True,
        text=True,
        check=False,
    )


def _arguments(log: Path) -> list[str]:
    """Return the arguments the runner stub recorded, one per line."""
    if not log.exists():
        return []
    return log.read_text(encoding="utf-8").splitlines()


def test_gate_passes_every_discovered_path_as_one_argument(tmp_path: Path) -> None:
    """Each discovered path reaches ``interrogate`` as exactly one argument.

    Scenario: the inventory holds a path with a space, one with a quote, one
    with a tab, a ``.py`` and a ``.pyi``. Invariant: the shell expansion keeps
    each path whole, so a whitespace split cannot turn one path into two
    arguments or drop it.
    """
    _build_fixture_repository(tmp_path)
    stub, log = _fake_runner(tmp_path, exit_status=0)
    result = _run_make(tmp_path, f"UV_RUN={stub}")
    assert result.returncode == 0, result.stderr
    arguments = _arguments(log)
    for relative in ALL_SCANNED:
        assert arguments.count(relative) == 1, (
            f"{relative!r} must reach interrogate exactly once; saw {arguments}"
        )


def test_gate_invokes_interrogate_with_the_documented_flags(
    tmp_path: Path,
) -> None:
    """The gate's single fixture-sized call carries every path and the flags.

    Scenario: the inventory holds several files. Invariant: the tool is asked
    for 100% with ``--ext pyi``, and every discovered path appears in the
    invocation. ``xargs`` may batch a very large inventory, so this asserts the
    flags and the inventory here, not an unconditional one-call guarantee: at
    a 100% threshold each batch must independently pass.
    """
    _build_fixture_repository(tmp_path)
    stub, log = _fake_runner(tmp_path, exit_status=0)
    result = _run_make(tmp_path, f"UV_RUN={stub}")
    assert result.returncode == 0, result.stderr
    arguments = _arguments(log)
    assert tuple(arguments[: len(GATE_ARGUMENTS)]) == GATE_ARGUMENTS, (
        f"the gate must run {GATE_ARGUMENTS}; it ran {arguments[: len(GATE_ARGUMENTS)]}"
    )
    scanned = [name for name in arguments if name in ALL_SCANNED]
    assert sorted(scanned) == sorted(ALL_SCANNED), (
        f"the invocation must carry the whole inventory; saw {scanned}"
    )


def test_gate_leaves_ignored_and_non_python_files_out(tmp_path: Path) -> None:
    """The scan stays confined to the inventory the guide describes.

    Scenario: the repository holds an ignored ``.py`` file and a tracked
    non-Python file. Invariant: neither reaches the scan, so the gate does not
    fail on files it never promised to cover.
    """
    _build_fixture_repository(tmp_path)
    stub, log = _fake_runner(tmp_path, exit_status=0)
    result = _run_make(tmp_path, f"UV_RUN={stub}")
    assert result.returncode == 0, result.stderr
    arguments = _arguments(log)
    assert IGNORED_PYTHON not in arguments, (
        f"an ignored file must stay outside the scan; saw {arguments}"
    )
    assert TRACKED_NON_PYTHON not in arguments, (
        f"a non-Python file must stay outside the scan; saw {arguments}"
    )
    # Without this, the two assertions above would also pass if the fixture
    # simply failed to create the files.
    assert (tmp_path / IGNORED_PYTHON).is_file()
    unignored = _run_git(
        tmp_path, "ls-files", "--others", "--", "*.*"
    ).stdout.splitlines()
    assert IGNORED_PYTHON in unignored, (
        f"the ignore rule must be what excludes it; saw {unignored}"
    )


def test_gate_fails_when_interrogate_fails(tmp_path: Path) -> None:
    """A non-zero ``interrogate`` verdict fails the target.

    Scenario: ``interrogate`` reports a coverage shortfall. Invariant: the
    target propagates the status, which is what makes it a gate rather than a
    report.
    """
    _build_fixture_repository(tmp_path)
    stub, _log = _fake_runner(tmp_path, exit_status=4)
    result = _run_make(tmp_path, f"UV_RUN={stub}")
    assert result.returncode != 0, (
        "`make docstring-coverage` must fail when interrogate fails"
    )


def test_gate_fails_when_discovery_fails(tmp_path: Path) -> None:
    """A failing ``git ls-files`` fails the target rather than passing it.

    Scenario: discovery fails before producing a path. Invariant: the recipe
    exits non-zero. A pipeline would report the status of its last command, so
    this is the case that would otherwise let an empty batch pass the gate.
    """
    _build_fixture_repository(tmp_path)
    stub, log = _fake_runner(tmp_path, exit_status=0)
    broken_git = tmp_path / "broken-git"
    broken_git.write_text("#!/bin/sh\nexit 128\n", encoding="utf-8")
    broken_git.chmod(broken_git.stat().st_mode | stat.S_IXUSR)
    result = _run_make(tmp_path, f"UV_RUN={stub}", f"GIT={broken_git}")
    assert result.returncode != 0, (
        "`make docstring-coverage` must fail when discovery fails; "
        f"stdout={result.stdout!r} stderr={result.stderr!r}"
    )
    assert _arguments(log) == [], (
        "no inventory means no interrogate invocation; "
        f"saw {_arguments(log)}"
    )


def test_gate_succeeds_on_a_genuinely_empty_inventory(tmp_path: Path) -> None:
    """An empty inventory is a pass, and does not run ``interrogate`` bare.

    Scenario: a repository holds no Python file at all. Invariant: the target
    succeeds and the tool is not invoked without paths. This is the valid
    counterpart to the failing-discovery case: absence of an inventory is not
    itself an error.
    """
    _run_git(tmp_path, "init", "--quiet", "--initial-branch", "main")
    _run_git(tmp_path, "config", "user.email", "contract@example.invalid")
    _run_git(tmp_path, "config", "user.name", "Contract")
    (tmp_path / "README.md").write_text("# empty\n", encoding="utf-8")
    _run_git(tmp_path, "add", ".")
    _run_git(tmp_path, "commit", "--quiet", "-m", "fixture")
    stub, log = _fake_runner(tmp_path, exit_status=0)
    result = _run_make(tmp_path, f"UV_RUN={stub}")
    assert result.returncode == 0, result.stderr
    assert _arguments(log) == [], (
        f"an empty inventory must not invoke interrogate; saw {_arguments(log)}"
    )


@pytest.mark.skipif(
    shutil.which("uv") is None, reason="the real-tool check needs uv on PATH"
)
def test_real_interrogate_fails_on_an_undocumented_file(tmp_path: Path) -> None:
    """A real ``interrogate`` run rejects the fixture's missing docstring.

    Scenario: the fixture's only undocumented file sits among fully documented
    ones, including a hidden-directory module and a ``.pyi`` stub. Invariant:
    the gate fails, and documenting that one file makes the same fixture pass.
    The second half is what attributes the failure to that file rather than to
    the fixture's awkward names, without depending on ``interrogate``'s output
    format.
    """
    deps = f"PYTHON_DEPS_FILE={REPOSITORY_ROOT / 'scripts/requirements-test.txt'}"
    _build_fixture_repository(tmp_path)
    failed = _run_make(tmp_path, deps)
    assert failed.returncode != 0, (
        "an undocumented Python file must fail the gate; "
        f"stdout={failed.stdout!r} stderr={failed.stderr!r}"
    )
    _write(tmp_path, UNDOCUMENTED_FILE, DOCUMENTED)
    repaired = _run_make(tmp_path, deps)
    assert repaired.returncode == 0, (
        "documenting the only undocumented file must make the fixture pass; "
        f"stdout={repaired.stdout!r} stderr={repaired.stderr!r}"
    )


def test_lint_keeps_the_docstring_gate_as_a_prerequisite() -> None:
    """``lint`` still depends on ``docstring-coverage``.

    Scenario: a contributor edits the prerequisite list. Invariant: the gate
    stays wired into the umbrella target, so ``make lint`` keeps enforcing it
    and not only the standalone target does.
    """
    rule = next(
        line
        for line in MAKEFILE.read_text(encoding="utf-8").splitlines()
        if line.startswith(f"{LINT_TARGET}:")
    )
    prerequisites = rule.split(":", 1)[1].split("##")[0].split()
    assert GATE_TARGET in prerequisites, (
        f"{LINT_TARGET} must depend on {GATE_TARGET}; its rule is {rule!r}"
    )
