"""Tests for the build-tree discard step run before coverage on Linux."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

SCRIPT_DIRECTORY = Path(__file__).resolve().parents[1]
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

from discard_build_trees import Removal, discard, main  # noqa: E402


def _tree(root: Path, name: str, size: int) -> None:
    """Create ``root/name`` holding one file of ``size`` bytes."""
    (root / name / "deps").mkdir(parents=True)
    (root / name / "deps" / "blob").write_bytes(b"x" * size)


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


@pytest.mark.parametrize("name", ["..", "../elsewhere", "/tmp"])
def test_a_name_outside_the_target_is_refused(tmp_path: Path, name: str) -> None:
    """The step cannot be pointed outside the target directory."""
    root = tmp_path / "target"
    root.mkdir()
    with pytest.raises(ValueError, match="resolves outside"):
        discard(root, [name])


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
