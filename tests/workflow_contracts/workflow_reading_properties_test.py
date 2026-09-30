"""Property: the step reading keeps every mapping step, in order, and nothing else.

`workflow_reading.workflow_steps` feeds the runner-placement contract. A
malformed job or step contributes nothing and raises nothing, wherever it sits
among well-formed ones, so generated workflows mix mapping steps with entries
that are not mappings and jobs that are not mappings.
"""

from __future__ import annotations

import typing as typ

from hypothesis import given, settings
from hypothesis import strategies as st
from workflow_reading import workflow_steps

#: Anything that is not a mapping, for the places a mapping belongs. The text
#: is drawn from a small alphabet because its content is not the subject.
_SHORT_TEXT: typ.Final[st.SearchStrategy[str]] = st.text(alphabet="ab: -", max_size=5)
MALFORMED: typ.Final[st.SearchStrategy[object]] = st.one_of(
    st.none(), _SHORT_TEXT, st.integers(), st.lists(_SHORT_TEXT, max_size=2)
)

_STEP: typ.Final[st.SearchStrategy[dict[str, object]]] = st.fixed_dictionaries(
    {"run": _SHORT_TEXT}
)


@st.composite
def _job(draw: st.DrawFn) -> tuple[object, list[dict[str, object]]]:
    """Draw one job and the mapping steps it holds, in order."""
    if draw(st.integers(min_value=0, max_value=5)) == 0:
        return draw(MALFORMED), []
    entries = draw(st.lists(st.one_of(_STEP, MALFORMED), max_size=4))
    kept = [entry for entry in entries if isinstance(entry, dict)]
    return {"steps": entries}, kept


@settings(max_examples=200)
@given(st.lists(_job(), max_size=4))
def test_the_step_reading_keeps_every_mapping_step_in_order(
    drawn: list[tuple[object, list[dict[str, object]]]],
) -> None:
    """Every mapping step, in declaration order, and nothing else."""
    document: dict[str | bool, object] = {
        "on": {"pull_request": ""},
        "jobs": {f"job{index}": job for index, (job, _) in enumerate(drawn)},
    }
    expected = [step for _, steps in drawn for step in steps]
    assert workflow_steps(document) == expected
