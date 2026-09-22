"""Properties of the step and token readings over generated workflows.

The tables in ``codescene_reader_test`` fix the finite spellings GitHub
accepts. These two readings are different in kind: their subject is an
arbitrary number of jobs and steps in arbitrary order, with malformed
fragments among them and the secret at any of several scopes, and no
table of handwritten documents covers that space.

The documents are workflow-shaped rather than free-form. A free-form
nested mapping almost never puts a value where these readers look, so a
property over one stays green with the readers' shape guards deleted;
here every generated value lands at a level the reader visits: the jobs
block, a job, its steps list, one step, and a step's ``env`` and
``with``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

from codescene_coverage import FORBIDDEN_VARIABLE
from codescene_reach import token_sites
from hypothesis import given, settings
from hypothesis import strategies as st
from workflow_reading import workflow_steps

#: How a step, job or workflow reads the secret, by scope.
READ: typ.Final[str] = f"${{{{ secrets.{FORBIDDEN_VARIABLE} }}}}"

#: Values that mention the secret without reading it. Each must be
#: ignored, so a reading that matched the bare name would report them.
DECOYS: typ.Final[list[str]] = [
    f"echo {FORBIDDEN_VARIABLE} is only set on main",
    "${{ secrets.OTHER_TOKEN }}",
    "",
]

#: Anything that is not a mapping, for the places a mapping belongs.
#: The text is drawn from a small alphabet because its content is not
#: the subject, and a full-Unicode ``st.text`` builds Hypothesis's
#: character tables on first use, slowly enough to trip the health
#: check on whichever property draws first.
_SHORT_TEXT: typ.Final[st.SearchStrategy[str]] = st.text(alphabet="ab: -", max_size=5)
MALFORMED: typ.Final[st.SearchStrategy[object]] = st.one_of(
    st.none(), _SHORT_TEXT, st.integers(), st.lists(_SHORT_TEXT, max_size=2)
)

#: The step scopes a step may read the secret through.
STEP_SCOPES: typ.Final[tuple[str, ...]] = ("env", "inputs", "run")


@st.composite
def _step(draw: st.DrawFn) -> tuple[dict[str, object], list[str]]:
    """Draw one mapping step and the scopes through which it reads the secret."""
    reads = [scope for scope in STEP_SCOPES if draw(st.booleans())]
    decoy = draw(st.sampled_from(DECOYS))
    step: dict[str, object] = {
        "env": {"TOKEN": READ} if "env" in reads else {"OTHER": decoy},
        "with": {"access-token": READ} if "inputs" in reads else {"note": decoy},
        "run": f"curl -H {READ}" if "run" in reads else decoy,
    }
    return step, reads


#: A step entry as drawn: the step and the scopes it reads through, or
#: ``None`` where a malformed entry stands in the steps list.
Entry = typ.Optional[tuple[dict[str, object], list[str]]]


@st.composite
def _job(draw: st.DrawFn) -> tuple[object, list[str], list[Entry]]:
    """Draw one job, the job-level sites it holds, and its step entries."""
    if draw(st.integers(min_value=0, max_value=5)) == 0:
        return draw(MALFORMED), [], []
    entries: list[Entry] = draw(st.lists(st.one_of(_step(), st.none()), max_size=4))
    job_sites = [scope for scope in ("env", "secrets: inherit") if draw(st.booleans())]
    job: dict[str, object] = {
        "env": {FORBIDDEN_VARIABLE: READ} if "env" in job_sites else {"A": DECOYS[0]},
        "steps": [entry[0] if entry else draw(MALFORMED) for entry in entries],
    }
    if "secrets: inherit" in job_sites:
        job["secrets"] = "inherit"
    return job, job_sites, entries


@st.composite
def _workflow(
    draw: st.DrawFn,
) -> tuple[dict[str | bool, object], list[str], list[dict[str, object]]]:
    """Draw a workflow, the token sites it holds and its mapping steps in order."""
    jobs = draw(st.lists(_job(), max_size=4))
    has_workflow_env = draw(st.booleans())
    document: dict[str | bool, object] = {
        "on": {"pull_request": ""},
        "env": {FORBIDDEN_VARIABLE: "x"} if has_workflow_env else {"A": DECOYS[0]},
        "jobs": {f"job{index}": job for index, (job, _, _) in enumerate(jobs)},
    }
    sites = ["ci.yml: workflow env"] if has_workflow_env else []
    steps: list[dict[str, object]] = []
    for index, (_, job_sites, entries) in enumerate(jobs):
        sites += [f"ci.yml: job job{index} {scope}" for scope in job_sites]
        for position, entry in enumerate(entries):
            if entry is None:
                continue
            step, reads = entry
            steps.append(step)
            where = f"ci.yml: job job{index} step {position + 1}"
            sites += [f"{where} {scope}" for scope in reads]
    return document, sites, steps


@settings(max_examples=200)
@given(_workflow())
def test_the_step_reading_keeps_every_mapping_step_in_order(
    drawn: tuple[dict[str | bool, object], list[str], list[dict[str, object]]],
) -> None:
    """Every mapping step, in declaration order, and nothing else.

    A malformed job or step contributes nothing and raises nothing,
    wherever it sits among well-formed ones.
    """
    document, _, steps = drawn
    assert workflow_steps(document) == steps


@settings(max_examples=200)
@given(_workflow())
def test_the_token_reading_reports_every_and_only_the_reads(
    drawn: tuple[dict[str | bool, object], list[str], list[dict[str, object]]],
) -> None:
    """Every scope that reads the secret is reported, in order, and no decoy.

    The expected sites are recorded as the document is drawn, at
    workflow, job env, ``secrets: inherit``, step env, step input and
    step script scope, across any
    number of jobs and steps in any order, with prose naming the
    variable and other secrets' references alongside.
    """
    document, sites, _ = drawn
    assert token_sites("ci.yml", document) == sites
