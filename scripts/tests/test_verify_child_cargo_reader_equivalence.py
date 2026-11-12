"""Behaviour of the child-cargo reader equivalence command.

``scripts/verify_child_cargo_reader_equivalence.py`` is the evidence that a
reader refactor preserved what the contract reads, so it has to be shown doing
all three things it claims: reporting agreement, reporting disagreement, and
refusing a revision that does not name a commit. A comparison that always said
"equal" would be indistinguishable from a correct one on the real history,
which is why the revisions here are controlled.

The repositories below are synthetic. Each revision holds a stub reader pair
whose class is fixed by the commit that carries it, so the only thing that can
make two revisions differ is the difference this test wrote.
"""

from __future__ import annotations

import pathlib
import subprocess
import sys

import pytest

SCRIPT_DIRECTORY = pathlib.Path(__file__).resolve().parents[1]
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

# The script directory is not a package, so the import must follow the path
# insertion above; E402 is expected here and nowhere else.
from verify_child_cargo_reader_equivalence import (  # noqa: E402
    _report,
    _source_differences,
)

SCRIPT = SCRIPT_DIRECTORY / "verify_child_cargo_reader_equivalence.py"

#: Where the command expects to find the reader helpers at each revision.
_READER_DIRECTORY = "tests/workflow_contracts"

#: A reader stub whose class is written into the revision that carries it.
#: ``CLASS_MEMBERS`` is substituted by :func:`_commit` rather than formatted,
#: so the stub's own braces need no escaping.
_STUB_TIER = '''"""A child-cargo reader whose class this revision decides."""

import pathlib

CHILD_CARGO_TIER_CRATES = ("crate_a",)


def _crate_test_roots(repository, crate):
    """Return one test root per crate, under ``repository``."""
    return [("alpha", pathlib.Path(repository) / crate / "tests" / "alpha.rs")]


def child_cargo_binaries(repository):
    """Return the class this revision reads."""
    return {CLASS_MEMBERS}


def non_child_cargo_binaries(repository):
    """Return the complement of the class, which the stub leaves empty."""
    return set()


def marker_binaries(repository):
    """Return which binary each marker selects."""
    return {"cargo-spawn": {CLASS_MEMBERS}}
'''

#: A module-graph stub reaching its own root and nothing else.
_STUB_GRAPH = '''"""A module-graph reader that reaches one root."""

import pathlib


def sources_reached(root):
    """Return the root's own text under the root's own path."""
    return {pathlib.Path(root): "fn run() {}\\n"}
'''


def _git(repository: pathlib.Path, *arguments: str) -> str:
    """Run one Git command in ``repository`` and return its stripped output."""
    return subprocess.run(
        ["git", "-C", str(repository), *arguments],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def _commit(repository: pathlib.Path, message: str, members: str) -> str:
    """Write the reader stubs reading ``members``, commit them, return the SHA."""
    readers = repository / _READER_DIRECTORY
    readers.mkdir(parents=True, exist_ok=True)
    (readers / "child_cargo_tier.py").write_text(
        _STUB_TIER.replace("CLASS_MEMBERS", members), encoding="utf-8"
    )
    (readers / "rust_module_graph.py").write_text(_STUB_GRAPH, encoding="utf-8")
    _git(repository, "add", "--all")
    # ``--allow-empty`` because the agreeing case deliberately writes the same
    # readers twice: the comparison must be shown reading two revisions whose
    # content matches, not two revisions whose content happens to differ.
    _git(repository, "commit", "--allow-empty", "--message", message)
    return _git(repository, "rev-parse", "HEAD")


def _run(repository: pathlib.Path, *arguments: str) -> subprocess.CompletedProcess[str]:
    """Invoke the command against ``repository`` and capture both streams."""
    return subprocess.run(
        [
            sys.executable,
            "-I",
            str(SCRIPT),
            *arguments,
            "--repository",
            str(repository),
        ],
        capture_output=True,
        text=True,
        check=False,
    )


@pytest.fixture
def repository(tmp_path: pathlib.Path) -> pathlib.Path:
    """Return an empty Git repository with an identity Git will commit under."""
    root = tmp_path / "repository"
    root.mkdir()
    _git(root, "init")
    _git(root, "config", "user.name", "Contract")
    _git(root, "config", "user.email", "contract@example.invalid")
    return root


def _snapshot(class_members: list[str], source_text: str) -> dict[str, object]:
    """Return one reader snapshot in the shape the worker serializes."""
    root = "crate_a/tests/alpha.rs"
    return {
        "roots": [["crate_a", "alpha", root]],
        "class": class_members,
        "non_class": [],
        "markers": {"cargo-spawn": class_members},
        "sources": {root: {root: source_text}},
    }


def test_identical_snapshots_pass_the_comparison(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A comparison of a snapshot with itself reports every reading equal."""
    snapshot = _snapshot(["alpha"], "fn run() {}\n")
    assert _report(snapshot, snapshot, "old", "new") is True, (
        "a snapshot compared with itself must not report a difference"
    )
    printed = capsys.readouterr().out
    assert "class equal: True 1" in printed, printed
    assert "roots differing (paths and text): 0" in printed, printed


def test_changed_source_text_is_counted_and_fails_the_comparison(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """One root whose reached text changed is one differing root, not zero.

    The class and the markers are identical here, so the comparison passes
    only if it also reads the sources. A reading that compared membership
    alone would report this pair as equal.
    """
    older = _snapshot(["alpha"], "fn run() {}\n")
    newer = _snapshot(["alpha"], "fn run() { let _ = 1; }\n")
    assert _report(older, newer, "old", "new") is False, (
        "a changed source text was not treated as a difference"
    )
    printed = capsys.readouterr().out
    assert "roots compared: 1" in printed, printed
    assert "roots differing (paths and text): 1" in printed, printed


def test_a_root_present_in_one_revision_only_is_counted_as_differing() -> None:
    """A root the older reading lacks is a difference, not a shared root."""
    older = _snapshot(["alpha"], "same")
    newer = _snapshot(["alpha"], "same")
    newer["sources"] = {}
    compared, differing = _source_differences(older, newer)
    assert compared == 0, f"no root is present in both readings, got {compared}"
    assert differing == 1, f"the missing root must count as differing, got {differing}"


def test_agreeing_revisions_exit_zero_and_report_each_reading(
    repository: pathlib.Path,
) -> None:
    """Two revisions reading the same class produce a zero exit."""
    older = _commit(repository, "read one member", '"alpha"')
    newer = _commit(repository, "read it again", '"alpha"')
    result = _run(repository, older, newer)
    assert result.returncode == 0, result.stdout + result.stderr
    assert "class equal: True 1" in result.stdout, result.stdout
    assert "markers equal: True" in result.stdout, result.stdout
    assert "root inventory equal: True" in result.stdout, result.stdout


def test_a_widened_class_exits_one_and_names_the_reading(
    repository: pathlib.Path,
) -> None:
    """A revision that reads one more member is reported, not passed.

    The stub changes the class in the one place the comparison is supposed to
    notice, so a zero exit here would mean the comparison never read it.
    """
    older = _commit(repository, "read one member", '"alpha"')
    newer = _commit(repository, "read two members", '"alpha", "beta"')
    result = _run(repository, older, newer)
    assert result.returncode == 1, result.stdout + result.stderr
    assert "class equal: False 2" in result.stdout, result.stdout
    assert "markers equal: False" in result.stdout, result.stdout
    assert "root inventory equal: True" in result.stdout, result.stdout


def test_an_unknown_revision_is_refused_as_a_usage_error(
    repository: pathlib.Path,
) -> None:
    """A name that resolves to no commit is a usage error, not a difference.

    Without this the command would fail with a traceback, and a mistyped
    revision would be indistinguishable from a reading that disagreed.
    """
    older = _commit(repository, "read one member", '"alpha"')
    result = _run(repository, older, "no-such-revision")
    assert result.returncode == 2, result.stdout + result.stderr
    assert "does not name a commit" in result.stderr, result.stderr


def test_worker_mode_refuses_a_truncated_invocation() -> None:
    """The worker refuses to run without both of its arguments."""
    result = subprocess.run(
        [sys.executable, "-I", str(SCRIPT), "--worker"],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0, "the worker ran with neither argument"
    assert "worker mode requires" in result.stderr, result.stderr