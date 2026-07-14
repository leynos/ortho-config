"""Test exact phrase-policy enforcement."""

import importlib
from pathlib import Path
import subprocess
import types
import pytest

SCRIPTS = Path(__file__).resolve().parents[1]
PROHIBITED = "hand" + "-written"
TITLE_PROHIBITED = "Hand" + "-written"


@pytest.fixture(name="modules")
def modules_fixture(
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[types.ModuleType, types.ModuleType]:
    monkeypatch.syspath_prepend(str(SCRIPTS))
    importlib.invalidate_caches()
    return importlib.import_module("typos_rollout"), importlib.import_module(
        "typos_rollout_check"
    )


def initialize(path: Path, files: dict[str, str]) -> None:
    for relative, content in files.items():
        target = path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content)
    subprocess.run(["git", "init", "--quiet"], cwd=path, check=True)
    subprocess.run(["git", "add", "."], cwd=path, check=True)


def test_phrase_merge_conflict(
    modules: tuple[types.ModuleType, types.ModuleType],
) -> None:
    rollout, _ = modules
    base = rollout.Dictionary(phrase_corrections=((PROHIBITED, "handwritten"),))
    assert (
        rollout.merge_dictionaries(base, rollout.Dictionary()).phrase_corrections
        == base.phrase_corrections
    )
    with pytest.raises(ValueError, match="conflicting phrase correction"):
        rollout.merge_dictionaries(
            base, rollout.Dictionary(phrase_corrections=((PROHIBITED, "other"),))
        )


def test_checker_boundaries_ignores_exclusions(
    modules: tuple[types.ModuleType, types.ModuleType], tmp_path: Path
) -> None:
    rollout, check = modules
    initialize(
        tmp_path,
        {
            "README.md": f"{PROHIBITED}\n{TITLE_PROHIBITED} prose\n`{PROHIBITED}`\n",
            "skip.md": f"{PROHIBITED}\n",
            "joined.md": "pre-hand" + "-written\n",
        },
    )
    policy = rollout.Dictionary(
        phrase_corrections=((PROHIBITED, "handwritten"),),
        ignore_patterns=(r"`[^`\n]+`",),
        excluded_files=("skip.md",),
    )
    assert [
        (x.line, x.phrase) for x in check.check_phrase_corrections(tmp_path, policy)
    ] == [(1, PROHIBITED), (2, TITLE_PROHIBITED)]


def test_main_reports(
    modules: tuple[types.ModuleType, types.ModuleType],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    rollout, check = modules
    initialize(tmp_path, {"README.md": f"Prefer {PROHIBITED}.\n"})
    monkeypatch.setattr(
        check.generator,
        "dictionary_from_cache",
        lambda _: rollout.Dictionary(phrase_corrections=((PROHIBITED, "handwritten"),)),
    )
    assert check.main(["--repository", str(tmp_path)]) == 2
    assert "README.md:1:8:" in capsys.readouterr().out
