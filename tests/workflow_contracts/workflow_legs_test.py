"""Examples for the Make-output seam in workflow reachability checks.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from build_standard_make import PREFLIGHT_SCRIPT
from workflow_legs import Leg, reaches_the_suite


def test_reaches_the_suite_uses_precomputed_make_output() -> None:
    """The query sends its goal and switches to the injected Make runner."""
    calls: list[tuple[str, list[str]]] = []

    def make_runner(target: str, switches: list[str]) -> list[str]:
        calls.append((target, switches))
        return [PREFLIGHT_SCRIPT]

    step = {"run": "make check-build-tools", "env": {"BUILD_STANDARD": 0}}
    leg = Leg("ci.yml", "build", {}, {"env": {"BUILD_STANDARD": "1"}})

    assert reaches_the_suite(step, leg, make_runner=make_runner)
    assert calls == [("check-build-tools", ["BUILD_STANDARD=0"])]


def test_reaches_the_suite_can_use_precomputed_lines() -> None:
    """A pure query can decide from lines without starting a Make process."""
    step = {"run": "make check-build-tools"}
    leg = Leg("ci.yml", "build", {}, {})

    assert not reaches_the_suite(step, leg, make_runner=lambda _goal, _switches: [])
