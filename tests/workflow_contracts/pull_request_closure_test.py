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
from codescene_coverage import called_workflows, pull_request_workflows
from codescene_reach import codescene_contacts, token_sites
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
    [pytest.param("./", id="dot-slash"), pytest.param("", id="root-relative")],
)
def test_the_lane_reaches_a_called_workflow(prefix: str) -> None:
    """A call is recognized by where it points, not by how it is spelt.

    Parametrised rather than combined, so each fails on its own and
    neither can be carried by the other. A reader enumerating accepted
    prefixes drops every spelling nobody thought to list; one that
    strips ``./`` and asks whether the rest is a file under the workflow
    directory reaches both of these.
    """
    caller = load_workflow(_caller(prefix))
    documents = {"ci.yml": caller, "probe.yml": load_workflow(PROBE)}
    assert called_workflows(caller, documents) == frozenset({"probe.yml"}), (
        f"the {prefix!r} spelling must resolve to the called workflow"
    )
    assert sorted(pull_request_workflows(documents)) == ["ci.yml", "probe.yml"], (
        "the called workflow runs on a pull request and belongs to that lane"
    )


@pytest.mark.parametrize(
    "reference",
    [
        pytest.param("./scripts/probe.yml", id="outside-the-workflow-directory"),
        pytest.param("./.github/workflows/nested/probe.yml", id="nested-directory"),
        pytest.param("./.github/workflows/absent.yml", id="no-such-workflow"),
    ],
)
def test_a_reference_of_the_wrong_shape_is_not_a_local_call(reference: str) -> None:
    """Assert the shape match is narrow as well as broad.

    Matching on the file name alone would read any path ending in
    ``probe.yml`` as a call to the workflow of that name, and GitHub
    calls nothing outside the workflow directory.
    """
    caller = load_workflow(f"on:\n  pull_request:\njobs:\n  call:\n    uses: {reference}\n")
    documents = {"ci.yml": caller, "probe.yml": load_workflow(PROBE)}
    assert called_workflows(caller, documents) == frozenset(), (
        f"{reference!r} does not name a workflow in this directory"
    )


def test_the_probe_is_caught_once_the_lane_includes_it() -> None:
    """The closure is only worth having if a clause then fails on it.

    This is the measurement the change rests on, asserted in both
    directions in one place: the trigger-only reading reaches `ci.yml`
    alone, and the closure puts the probe in reach of the secret clause
    along with the caller's own ``secrets: inherit``, and of the host
    clause. The two clauses travel together: run over a trigger list,
    they would share one blind spot while each looked like it covered
    the other.
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
    contacts = sorted(
        site
        for name, document in pull_request_workflows(documents).items()
        for site in codescene_contacts(name, document)
    )
    assert contacts == ["probe.yml: jobs.probe.steps[0].run"], (
        f"the host clause runs over the same closure, so the probe's curl "
        f"is caught there too; it found {contacts}"
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


#: A reusable workflow that takes the host as an input and uses it. It
#: names no host itself, so only the caller's ``with`` carries it.
CONSUMER: typ.Final[str] = (
    "on:\n  workflow_call:\n    inputs:\n      url:\n        type: string\n"
    "jobs:\n  use:\n    steps:\n"
    "      - run: curl \"${{ inputs.url }}\"\n"
)


def test_a_host_passed_to_a_called_workflow_is_caught_at_the_call() -> None:
    """The host can reach a step through a reusable workflow's input.

    The called workflow names no host, so a reading of steps alone finds
    nothing in either file; the URL sits in the caller's job-level
    ``with``, which is where the clause has to look.
    """
    caller = load_workflow(
        "on:\n  pull_request:\n"
        "jobs:\n  call:\n"
        "    uses: ./.github/workflows/consumer.yml\n"
        "    with:\n      url: https://api.codescene.io/v2/projects/1\n"
    )
    documents = {"ci.yml": caller, "consumer.yml": load_workflow(CONSUMER)}
    contacts = sorted(
        site
        for name, document in pull_request_workflows(documents).items()
        for site in codescene_contacts(name, document)
    )
    assert contacts == ["ci.yml: jobs.call.with.url"], (
        f"the caller's `with` carries the host into the lane; found {contacts}"
    )


@pytest.mark.parametrize(
    ("body", "where"),
    [
        pytest.param(
            "env:\n  URL: https://codescene.io\njobs:\n  a:\n    steps:\n"
            '      - run: curl "$URL"\n',
            "env.URL",
            id="workflow-env",
        ),
        pytest.param(
            "jobs:\n  a:\n    env:\n      URL: https://codescene.io\n"
            '    steps:\n      - run: curl "$URL"\n',
            "jobs.a.env.URL",
            id="job-env",
        ),
        pytest.param(
            "jobs:\n  a:\n    steps:\n      - env:\n          URL: https://codescene.io\n"
            '        run: curl "$URL"\n',
            "jobs.a.steps[0].env.URL",
            id="step-env",
        ),
        pytest.param(
            "jobs:\n  a:\n    steps:\n      - uses: some/action@v1\n"
            "        with:\n          url: https://codescene.io\n",
            "jobs.a.steps[0].with.url",
            id="step-input",
        ),
        pytest.param(
            "jobs:\n  a:\n    services:\n      s:\n        image: x\n"
            "        env:\n          URL: https://codescene.io\n",
            "jobs.a.services.s.env.URL",
            id="service-env",
        ),
    ],
)
def test_the_host_clause_reads_every_scope(body: str, where: str) -> None:
    """Every scope a URL can reach a process from is read.

    Enumerating the step's script, inputs and environment left the
    workflow's and the job's ``env`` as a way round the rule: a step
    inherits both. Reading every value closes the class rather than the
    instances found so far.
    """
    contacts = codescene_contacts("ci.yml", load_workflow(f"on:\n  pull_request:\n{body}"))
    assert contacts == [f"ci.yml: {where}"], f"expected the host at {where}; found {contacts}"


def test_the_host_clause_ignores_a_comment() -> None:
    """Assert the clause is narrow: prose in a comment is not a contact.

    The workflows explain in comments why CodeScene is off this lane,
    and those comments name the service.
    """
    body = "on:\n  pull_request:\n# the check moved off codescene.io\njobs:\n  a:\n    steps: []\n"
    assert codescene_contacts("ci.yml", load_workflow(body)) == []
