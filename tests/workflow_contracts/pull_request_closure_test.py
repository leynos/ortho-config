"""The pull-request lane as a closure, not a trigger list.

A workflow declaring only ``workflow_call`` still runs on a pull request
when a pull-request workflow calls it, and ``secrets: inherit`` hands it
the token. A reading that enumerated triggers alone could not see it, so
every refusal built on that enumeration passed over it while it did the
forbidden thing.

Measured on episodic rather than argued: a probe of the shape below
passed every clause of the equivalent contract there.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import pytest
from codescene_coverage import called_workflows, pull_request_workflows, token_sites
from workflow_reading import load_workflow, serves_pull_requests

#: A workflow that declares only ``workflow_call`` and reaches CodeScene
#: with whatever secret it was handed. It names no CodeScene action and
#: runs no ``cs-coverage``, so those clauses are blind to it either way;
#: the token clause catches it, and only once the closure puts it in the
#: lane at all.
PROBE: typ.Final[str] = (
    "on:\n  workflow_call:\n"
    "jobs:\n  probe:\n    steps:\n"
    "      - run: |\n"
    '          curl -H "Authorization: ${{ secrets.CS_ACCESS_TOKEN }}" \\\n'
    "            https://api.codescene.io/v2/projects/69279\n"
)


def _caller(prefix: str) -> str:
    """Return a pull-request workflow calling the probe by one spelling."""
    return (
        "on:\n  pull_request:\n"
        "jobs:\n  call:\n"
        f"    uses: {prefix}.github/workflows/probe.yml\n"
        "    secrets: inherit\n"
    )


@pytest.mark.parametrize(
    "prefix",
    [pytest.param("./", id="dot-slash"), pytest.param("$/", id="dollar-slash")],
)
def test_the_lane_reaches_a_called_workflow(prefix: str) -> None:
    """Both spellings GitHub accepts reach the same file.

    Parametrised rather than combined, so each fails on its own and
    neither can be carried by the other. A reader knowing only ``./``
    silently drops callers written the other way, and the documented
    recommendation is the one it would drop.
    """
    caller = load_workflow(_caller(prefix))
    documents = {"ci.yml": caller, "probe.yml": load_workflow(PROBE)}
    assert called_workflows(caller, documents) == frozenset({"probe.yml"}), (
        f"the {prefix!r} spelling must resolve to the called workflow"
    )
    assert sorted(pull_request_workflows(documents)) == ["ci.yml", "probe.yml"], (
        "the called workflow runs on a pull request and belongs to that lane"
    )


def test_the_probe_is_caught_once_the_lane_includes_it() -> None:
    """The closure is only worth having if a clause then fails on it.

    This is the measurement the change rests on, asserted in both
    directions in one place: the trigger-only reading reaches `ci.yml`
    alone, and the closure puts the probe in reach of the secret clause
    along with the caller's own ``secrets: inherit``.
    """
    documents = {
        "ci.yml": load_workflow(_caller("./")),
        "probe.yml": load_workflow(PROBE),
    }
    trigger_only = [
        name for name, document in documents.items() if serves_pull_requests(document)
    ]
    assert trigger_only == ["ci.yml"], (
        "the premise: enumerating triggers alone does not reach the probe"
    )
    offenders = sorted(
        site
        for name, document in pull_request_workflows(documents).items()
        for site in token_sites(name, document)
    )
    assert offenders == [
        "ci.yml: job call secrets: inherit",
        "probe.yml: job probe step 1 run",
    ], (
        f"the closure must put the probe in reach of the secret clause, and "
        f"the caller's `secrets: inherit` with it; it found {offenders}"
    )


def test_a_workflow_nothing_calls_is_not_in_the_lane() -> None:
    """Assert the closure is narrow as well as transitive.

    A traversal sweeping in every ``workflow_call`` document, rather
    than the ones a pull-request workflow actually calls, would hold
    workflows the lane never runs to the lane's rules and fail a
    repository that complies.
    """
    documents = {
        "ci.yml": load_workflow("on:\n  pull_request:\njobs:\n  a:\n    steps: []\n"),
        "probe.yml": load_workflow(PROBE),
    }
    assert sorted(pull_request_workflows(documents)) == ["ci.yml"], (
        "a reusable workflow no pull-request lane calls is not in the lane"
    )


def test_a_call_to_another_repository_is_not_followed() -> None:
    """What is not in this tree cannot be read, and is not claimed to be.

    Following a cross-repository reference would mean asserting over
    content this reading does not have. Saying plainly that it is out of
    scope is better than a silent pass that looks like coverage.
    """
    caller = load_workflow(
        "on:\n  pull_request:\n"
        "jobs:\n  call:\n"
        "    uses: other/repo/.github/workflows/probe.yml@main\n"
    )
    documents = {"ci.yml": caller, "probe.yml": load_workflow(PROBE)}
    assert called_workflows(caller, documents) == frozenset(), (
        "a cross-repository call names a document this reading does not hold"
    )
