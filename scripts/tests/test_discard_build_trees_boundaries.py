"""Boundary tests for the build-tree discard step: failures, the process, safety.

``test_discard_build_trees.py`` covers the happy path and the named-tree
refusals. This module covers what CI actually depends on: every filesystem
failure surfacing as one diagnostic, the script run as a process, the
partial-progress guarantee, and a property over arbitrary names.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

SCRIPT_DIRECTORY = Path(__file__).resolve().parents[1]
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

# The script directory is not a package, so the import must follow the path
# insertion above; E402 is expected here and nowhere else.
import discard_build_trees  # noqa: E402
from discard_build_trees import DiscardError, discard, main, tree_size  # noqa: E402

SCRIPT = SCRIPT_DIRECTORY / "discard_build_trees.py"


def _tree(root: Path, name: str, size: int = 1) -> None:
    """Create ``root/name`` holding one sparse file of ``size`` logical bytes."""
    (root / name / "deps").mkdir(parents=True)
    with (root / name / "deps" / "blob").open("wb") as blob:
        blob.truncate(size)


def _run(*arguments: str | Path) -> subprocess.CompletedProcess[str]:
    """Run the script as CI does and capture its streams as text."""
    return subprocess.run(  # noqa: S603 - fixed interpreter and script, test-owned arguments
        [sys.executable, str(SCRIPT), *map(str, arguments)],
        capture_output=True,
        text=True,
        check=False,
    )


def test_a_missing_target_directory_fails_rather_than_reporting_absence(tmp_path: Path) -> None:
    """A wrong target path must not let the step pass having freed nothing."""
    with pytest.raises(DiscardError, match="not a directory"):
        discard(tmp_path / "absent", ["debug"])


@pytest.mark.parametrize("name", ["debug/deps", "ABSOLUTE"])
def test_a_nested_or_absolute_name_is_refused(tmp_path: Path, name: str) -> None:
    """Only a single relative component naming a direct child is accepted.

    ``ABSOLUTE`` is the direct child ``debug`` spelt as an absolute path, which
    resolves inside the target yet is not the documented form.
    """
    _tree(tmp_path, "debug")
    requested = str(tmp_path / "debug") if name == "ABSOLUTE" else name
    with pytest.raises(ValueError, match="single relative name"):
        discard(tmp_path, [requested])
    assert (tmp_path / "debug" / "deps" / "blob").exists(), "nothing may be removed"


def test_a_directory_scan_error_is_not_swallowed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A child directory that cannot be scanned fails the measurement.

    ``os.walk`` drops scan errors unless given ``onerror``; the fake drives that
    callback directly, so the test needs no permission bits.
    """
    _tree(tmp_path, "debug")

    def unreadable(_top: Path, onerror: object, **_: object) -> object:
        onerror(PermissionError("simulated scan failure"))  # type: ignore[operator]
        return iter(())

    monkeypatch.setattr(discard_build_trees.os, "walk", unreadable)
    with pytest.raises(DiscardError, match="cannot measure") as raised:
        tree_size(tmp_path / "debug")
    assert isinstance(raised.value.__cause__, PermissionError), "the cause must be kept"


def test_a_resolution_failure_is_typed(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A symlink loop or I/O error while resolving is a ``DiscardError``."""

    def refuse(_self: Path, **_: object) -> Path:
        message = "simulated resolution failure"
        raise OSError(message)

    monkeypatch.setattr(Path, "resolve", refuse)
    with pytest.raises(DiscardError, match="cannot resolve"):
        discard(tmp_path, ["debug"])


def test_an_examination_failure_is_not_taken_for_absence(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """A permission error on the tree fails the step instead of reporting it absent."""
    _tree(tmp_path, "debug")
    real_lstat = Path.lstat

    def refuse(self: Path) -> object:
        if self.name == "debug":
            message = "simulated permission failure"
            raise PermissionError(message)
        return real_lstat(self)

    monkeypatch.setattr(Path, "lstat", refuse)
    assert main([str(tmp_path), "debug"]) == 1, "the step must fail"
    assert "cannot examine" in capsys.readouterr().err, "the diagnostic must name the action"


def test_a_later_failure_leaves_earlier_trees_removed_and_later_ones_alone(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The documented partial-progress guarantee: stop at the failure, no further."""
    for name in ("first", "second", "third"):
        _tree(tmp_path, name)
    real_rmtree = discard_build_trees.shutil.rmtree

    def fail_on_second(path: Path) -> None:
        if Path(path).name == "second":
            message = "simulated removal failure"
            raise OSError(message)
        real_rmtree(path)

    monkeypatch.setattr(discard_build_trees.shutil, "rmtree", fail_on_second)
    with pytest.raises(DiscardError, match="cannot remove"):
        discard(tmp_path, ["first", "second", "third"])
    assert not (tmp_path / "first").exists(), "the tree before the failure must stay removed"
    assert (tmp_path / "second").exists(), "the failed tree must remain"
    assert (tmp_path / "third").exists(), "no tree after the failure may be touched"


def test_the_process_reports_and_removes(tmp_path: Path) -> None:
    """Run as a process, the script prints one line per name and exits 0."""
    _tree(tmp_path, "debug")
    result = _run(tmp_path, "debug", "dylint")
    assert result.returncode == 0, f"unexpected failure: {result.stderr}"
    assert result.stdout.splitlines() == [
        "debug: removed 0.00 GiB",
        "dylint: nothing to remove",
    ], "each name must report its outcome on stdout"
    assert result.stderr == "", "a success must be silent on stderr"
    assert not (tmp_path / "debug").exists(), "the named tree must be gone"


def test_the_process_without_arguments_is_a_usage_error() -> None:
    """The real ``sys.argv`` entry point gives status 2 and a usage line."""
    result = _run()
    assert result.returncode == 2, "a missing target directory must fail with 2"
    assert result.stdout == "", "nothing may be reported as removed"
    assert result.stderr.startswith("usage: "), "the usage line goes to stderr"


def test_the_process_refuses_an_escape_without_a_traceback(tmp_path: Path) -> None:
    """A refused name exits 1 with one diagnostic line and touches nothing."""
    root = tmp_path / "target"
    _tree(root, "debug")
    result = _run(root, "..", "debug")
    assert result.returncode == 1, "a refusal must fail the step"
    assert result.stderr.startswith("discard failed: "), "the diagnostic must be on stderr"
    assert "Traceback" not in result.stderr, "the refusal must not be a traceback"
    assert (root / "debug").exists(), "names after a refusal must not be processed"


_COMPONENTS = st.one_of(
    st.sampled_from(["debug", "doc", "dylint", "..", ".", "", "a/b", "../x", "debug/deps"]),
    st.text(alphabet="abc./", min_size=0, max_size=6),
)


# deadline=None: every example makes a temporary directory, so a deadline would
# assert the host's disk speed rather than the safety property.
@settings(deadline=None)
@given(names=st.lists(_COMPONENTS, max_size=5))
def test_no_name_can_delete_outside_the_named_direct_children(names: list[str]) -> None:
    """For any names, nothing outside the target, and not the target, is removed.

    The invariant is the safety contract itself: a sentinel beside the target
    survives, the target directory survives, and a returned result names
    exactly the requested names in order.
    """
    with tempfile.TemporaryDirectory() as scratch:
        parent = Path(scratch)
        root = parent / "target"
        root.mkdir()
        sentinel = parent / "sentinel"
        sentinel.write_bytes(b"keep")
        (root / "debug").mkdir()
        try:
            removals = discard(root, names)
        except (ValueError, DiscardError):
            removals = None
        assert sentinel.exists(), "a file outside the target must survive"
        assert root.is_dir(), "the target directory itself must survive"
        if removals is not None:
            assert [removal.name for removal in removals] == names, "one result per name, in order"
