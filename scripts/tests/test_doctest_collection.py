"""Guard the doctest list in the Makefile against drift.

`PYTEST_FLAGS` names the modules `--doctest-modules` collects. The list
is written by hand, so it drifts from the tree in both directions: a
module that gains an ``Examples`` section is collected only if somebody
remembers to add it, and a module the list still names after a deletion
ends the whole lane. Both have happened here. The spelling helper
carried an example that nothing ran until it was named, and it was then
deleted with the legacy generator while the list still named it. The
contracts below are the reason neither can recur silently.
"""

from __future__ import annotations

import pathlib
import re
import typing as typ

REPOSITORY: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
MAKEFILE: typ.Final[pathlib.Path] = REPOSITORY / "Makefile"
SCRIPTS: typ.Final[pathlib.Path] = REPOSITORY / "scripts"

#: ``PYTEST_FLAGS`` and its backslash continuations, up to the line that
#: does not end in one.
_ASSIGNMENT: typ.Final[re.Pattern[str]] = re.compile(
    r"^PYTEST_FLAGS\s*\?=\s*((?:.*\\\n)*.*)$", re.MULTILINE
)


def _doctest_paths() -> frozenset[str]:
    """Return the paths ``PYTEST_FLAGS`` hands to ``--doctest-modules``."""
    # Both reads are asserted rather than tolerated: without the
    # assignment, or without the flag, every contract in this module
    # would pass over an empty set.
    match = _ASSIGNMENT.search(MAKEFILE.read_text(encoding="utf-8"))
    assert match is not None, "PYTEST_FLAGS is not assigned in the Makefile"
    words = match.group(1).replace("\\\n", " ").split()
    assert "--doctest-modules" in words, "PYTEST_FLAGS does not collect doctests"
    return frozenset(word for word in words if not word.startswith("-"))


def _modules_with_examples() -> frozenset[pathlib.Path]:
    """Return repository-relative script paths that carry an example."""
    return frozenset(
        path.relative_to(REPOSITORY)
        for path in sorted(SCRIPTS.rglob("*.py"))
        if ".venv" not in path.parts and ">>>" in path.read_text(encoding="utf-8")
    )


def test_every_script_with_an_example_is_collected() -> None:
    """Assert the list in the Makefile covers every module with examples.

    A module is covered either by its own name or by an ancestor
    directory the list names, which is how `scripts/tests` covers the
    test modules. The failure this guards is a module gaining an example
    and nobody adding it, so the example is never executed and can go
    untrue without a gate noticing.
    """
    collected = _doctest_paths()
    uncollected = sorted(
        str(module)
        for module in _modules_with_examples()
        if str(module) not in collected
        and not any(str(parent) in collected for parent in module.parents)
    )
    assert not uncollected, (
        "these modules carry docstring examples that PYTEST_FLAGS does not "
        f"collect: {', '.join(uncollected)}"
    )


def test_the_list_names_no_module_that_is_gone() -> None:
    """Assert every named path still exists.

    A path that has been renamed or deleted makes pytest fail on the
    whole lane rather than silently collecting less, so this is the
    cheaper end of the same drift.
    """
    missing = sorted(
        path for path in _doctest_paths() if not (REPOSITORY / path).exists()
    )
    assert not missing, f"PYTEST_FLAGS names paths that do not exist: {missing}"


def test_the_sweep_finds_the_modules_that_carry_examples() -> None:
    """Assert the discovery is not empty, and names a module it must find.

    Both contracts above are satisfied by a sweep that returns nothing,
    so the sweep itself is pinned: `scripts/bump_version.py` carries
    fifty-odd examples and is the module a working discovery cannot
    miss.
    """
    modules = _modules_with_examples()
    assert modules, "the sweep found no module carrying a docstring example"
    assert pathlib.Path("scripts/bump_version.py") in modules
