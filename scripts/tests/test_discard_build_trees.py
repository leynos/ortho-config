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
from discard_build_trees import Removal, discard, main  # noqa: E402


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
