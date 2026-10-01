"""Contract for the ``spelling`` alias of the spelling gate.

The shared ``AGENTS.md`` spelling block tells agents to run ``make spelling``,
but this repository's gate target is ``spellcheck``, which ``make
markdownlint`` and CI run. The ``spelling`` target exists only so that the
command the block names is real. This module asserts both ends: the alias must
reach the gate, and the gate must still be the pinned
``typos-config-builder gate``. An alias that stopped depending on the gate
would leave ``make spelling`` succeeding while checking nothing.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

from makefile_support import recipe_lines

MAKEFILE_PATH = Path(__file__).resolve().parents[2] / "Makefile"

#: The alias and the target it must reach.
ALIAS: typ.Final[str] = "spelling"
GATE_TARGET: typ.Final[str] = "spellcheck"


def _makefile() -> str:
    """Return the repository Makefile text."""
    return MAKEFILE_PATH.read_text(encoding="utf-8")


def test_spelling_alias_depends_on_the_gate_target() -> None:
    """``make spelling`` runs the gate by depending on ``spellcheck``.

    Scenario: the AGENTS.md block names ``make spelling``. Invariant: that
    target lists ``spellcheck`` as a prerequisite, so running it runs the gate.
    """
    rule = re.search(rf"^{ALIAS}\s*:(?!=)([^\n#]*)", _makefile(), re.MULTILINE)
    assert rule is not None, f"the Makefile must define a `{ALIAS}` target"
    assert GATE_TARGET in rule.group(1).split(), (
        f"`{ALIAS}` must depend on `{GATE_TARGET}` so it runs the gate"
    )


def test_spelling_alias_is_phony() -> None:
    """The alias is declared ``.PHONY`` so a file named ``spelling`` cannot skip it."""
    phony = re.search(r"^\.PHONY:(.*(?:\\\n.*)*)", _makefile(), re.MULTILINE)
    assert phony is not None, "the Makefile must declare .PHONY targets"
    assert ALIAS in phony.group(1).replace("\\\n", " ").split(), (
        f"`{ALIAS}` must be .PHONY"
    )


def test_gate_target_still_runs_the_pinned_builder_gate() -> None:
    """The target the alias reaches still invokes the builder's ``gate`` command."""
    recipe = recipe_lines(_makefile(), GATE_TARGET)
    assert any("$(TYPOS_CONFIG_BUILDER) gate" in line for line in recipe), (
        f"`{GATE_TARGET}` must run `$(TYPOS_CONFIG_BUILDER) gate`"
    )
