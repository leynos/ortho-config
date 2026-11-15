"""Examples for the workflow-leg readings the provisioning contract relies on.

The matrix reader's ``include`` behaviour is checked against GitHub's own
documented example, and the Make seam is checked for the goal and the switches
it passes. Properties over generated matrices live in
``workflow_legs_properties_test.py``; these are the readable cases.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from build_standard_make import PREFLIGHT_SCRIPT
from workflow_legs import Leg, legs, reaches_the_suite


def test_matrix_include_augments_cells_as_github_documents() -> None:
    """The documented ``include`` example yields exactly the documented cells.

    GitHub's own example combines an added key, an added key restricted by an
    axis value, an axis value restricted by an added key, and two entries that
    match no product cell and so become cells of their own.
    """
    job = {
        "strategy": {
            "matrix": {
                "fruit": ["apple", "pear"],
                "animal": ["cat", "dog"],
                "include": [
                    {"color": "green"},
                    {"color": "pink", "animal": "cat"},
                    {"fruit": "apple", "shape": "circle"},
                    {"fruit": "banana"},
                    {"fruit": "banana", "animal": "cat"},
                ],
            }
        }
    }

    assert [leg.cell for leg in legs("ci.yml", "build", job)] == [
        {"fruit": "apple", "animal": "cat", "color": "pink", "shape": "circle"},
        {"fruit": "apple", "animal": "dog", "color": "green", "shape": "circle"},
        {"fruit": "pear", "animal": "cat", "color": "pink"},
        {"fruit": "pear", "animal": "dog", "color": "green"},
        {"fruit": "banana"},
        {"fruit": "banana", "animal": "cat"},
    ]


def test_a_command_line_assignment_outranks_the_step_environment() -> None:
    """``make test BUILD_STANDARD=0`` turns the standard off despite the export."""
    seen: list[list[str]] = []

    def make_runner(_target: str, switches: list[str]) -> list[str]:
        """Record the switches the query would pass to Make."""
        seen.append(switches)
        return []

    step = {"run": "make test BUILD_STANDARD=0", "env": {"BUILD_STANDARD": "1"}}

    assert not reaches_the_suite(step, Leg("ci.yml", "build", {}, {}), make_runner=make_runner)
    assert seen == [["BUILD_STANDARD=0"]]


def test_reaches_the_suite_uses_precomputed_make_output() -> None:
    """The query sends its goal and switches to the injected Make runner."""
    calls: list[tuple[str, list[str]]] = []

    def make_runner(target: str, switches: list[str]) -> list[str]:
        """Record the query's goal and switches, then report the preflight step."""
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
