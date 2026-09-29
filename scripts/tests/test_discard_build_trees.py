"""Tests for the build-tree discard step run before coverage on Linux."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

SCRIPT_DIRECTORY = Path(__file__).resolve().parents[1]
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

# The script directory is not a package, so the import must follow the path
# insertion above; E402 is expected here and nowhere else.
from discard_build_trees import DiscardError, Removal, discard, main, tree_size  # noqa: E402


def fail_removal(monkeypatch: pytest.MonkeyPatch) -> None:
    """Make every ``shutil.rmtree`` call raise ``OSError``."""

    def refuse(_path: Path) -> None:
        message = "simulated removal failure"
        raise OSError(message)

    monkeypatch.setattr("discard_build_trees.shutil.rmtree", refuse)


def _tree(root: Path, name: str, size: int) -> None:
    """Create ``root/name`` holding one sparse file of ``size`` logical bytes."""
    (root / name / "deps").mkdir(parents=True)
    with (root / name / "deps" / "blob").open("wb") as blob:
        blob.truncate(size)


def test_named_trees_are_removed_and_sized(tmp_path: Path) -> None:
    """Each named tree goes, with its size reported; others stay."""
    _tree(tmp_path, "debug", 100)
    _tree(tmp_path, "llvm-cov-target", 7)
    removals = discard(tmp_path, ["debug"])
    assert removals == [Removal("debug", 100)], "debug must be sized and removed"
    assert not (tmp_path / "debug").exists(), "debug must be gone"
    assert (tmp_path / "llvm-cov-target").exists(), "an unnamed tree must stay"


def test_an_absent_tree_is_reported_not_skipped(tmp_path: Path) -> None:
    """A name with nothing on disk is reported as nothing to remove."""
    assert discard(tmp_path, ["dylint"]) == [Removal("dylint", None)], (
        "an absent tree must be reported with no size"
    )


@pytest.mark.parametrize("name", ["..", "../elsewhere", "ABSOLUTE"])
def test_a_name_outside_the_target_is_refused(tmp_path: Path, name: str) -> None:
    """The step cannot be pointed outside the target directory.

    ``ABSOLUTE`` stands for an absolute path outside the target, built from
    ``tmp_path`` so the case needs no fixed system directory.
    """
    root = tmp_path / "target"
    root.mkdir()
    requested = str(tmp_path / "elsewhere") if name == "ABSOLUTE" else name
    with pytest.raises(ValueError, match="resolves outside"):
        discard(root, [requested])


def test_a_symlink_alias_is_refused_and_its_target_kept(tmp_path: Path) -> None:
    """A link such as ``debug -> llvm-cov-target`` deletes nothing."""
    _tree(tmp_path, "llvm-cov-target", 7)
    (tmp_path / "debug").symlink_to(tmp_path / "llvm-cov-target")
    with pytest.raises(ValueError, match="not a plain path"):
        discard(tmp_path, ["debug"])
    assert (tmp_path / "llvm-cov-target" / "deps" / "blob").exists(), (
        "the tree behind the alias must survive"
    )


def test_the_command_prints_one_line_per_name(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """The log says what went and what was already absent."""
    _tree(tmp_path, "doc", 2**30)
    assert main([str(tmp_path), "doc", "dylint"]) == 0, "the command must succeed"
    assert capsys.readouterr().out.splitlines() == [
        "doc: removed 1.00 GiB",
        "dylint: nothing to remove",
    ], "each name must report its outcome"


def test_an_unreadable_file_raises_a_typed_error(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """An ``lstat`` failure surfaces as ``DiscardError``, not a bare ``OSError``."""
    _tree(tmp_path, "debug", 1)

    def refuse(_self: Path, **_: object) -> object:
        message = "simulated stat failure"
        raise PermissionError(message)

    monkeypatch.setattr(Path, "lstat", refuse)
    with pytest.raises(DiscardError, match="cannot measure") as raised:
        tree_size(tmp_path / "debug")
    assert isinstance(raised.value.__cause__, PermissionError), "the cause must be kept"


def test_a_failed_removal_raises_a_typed_error(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """An ``rmtree`` failure surfaces as ``DiscardError`` naming the tree."""
    _tree(tmp_path, "debug", 1)

    fail_removal(monkeypatch)
    with pytest.raises(DiscardError, match="cannot remove"):
        discard(tmp_path, ["debug"])


@pytest.mark.parametrize("name", ["..", "debug-link"])
def test_the_command_reports_a_refusal_and_fails(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], name: str
) -> None:
    """A refused name gives one diagnostic line and status 1, with no traceback."""
    root = tmp_path / "target"
    _tree(root, "real", 1)
    (root / "debug-link").symlink_to(root / "real")
    assert main([str(root), name]) == 1, "a refusal must fail the step"
    captured = capsys.readouterr()
    assert captured.out == "", "nothing may be reported as removed"
    assert captured.err.startswith("discard failed: "), "the diagnostic must be on stderr"
    assert (root / "real").exists(), "the tree behind the alias must survive"


def test_the_command_reports_a_filesystem_failure_and_fails(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], monkeypatch: pytest.MonkeyPatch
) -> None:
    """A failed removal gives one diagnostic line and status 1."""
    _tree(tmp_path, "debug", 1)

    fail_removal(monkeypatch)
    assert main([str(tmp_path), "debug"]) == 1, "a failure must fail the step"
    assert "cannot remove" in capsys.readouterr().err, "the diagnostic must name the action"


def test_the_command_without_a_target_prints_usage(capsys: pytest.CaptureFixture[str]) -> None:
    """An empty command line is a usage error, not an unpacking traceback."""
    assert main([]) == 2, "a missing target directory must fail the step"
    assert capsys.readouterr().err.startswith("usage: "), "the usage line goes to stderr"


def test_a_name_that_is_the_target_itself_is_refused(tmp_path: Path) -> None:
    """``.`` would remove the whole target directory, so it is refused."""
    with pytest.raises(ValueError, match="target directory"):
        discard(tmp_path, ["."])
    assert tmp_path.exists(), "the target directory must survive"


def test_a_symlink_inside_a_tree_is_not_counted(tmp_path: Path) -> None:
    """A link counts for nothing, so the size matches what ``rmtree`` frees."""
    _tree(tmp_path, "debug", 5)
    outside = tmp_path / "outside"
    with outside.open("wb") as blob:
        blob.truncate(1000)
    (tmp_path / "debug" / "link").symlink_to(outside)
    assert tree_size(tmp_path / "debug") == 5, "only the tree's own file counts"
