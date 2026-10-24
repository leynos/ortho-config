"""Properties of the workflow-leg interpreter that the provisioning contract relies on.

``workflow_legs`` decides which jobs run on Linux and which steps reach the
suite, and ``build_tools_provisioning_test.py`` demands an installer of every
such leg. A wrong interpreter makes that contract pass vacuously or fail for the
wrong job, and the repository's own workflows use one spelling of everything, so
they cannot tell a working interpreter from a broken one. These properties
generate the shapes the real workflows do not contain.

Each property is a plain ``check_*`` function over an interpreter argument, run
twice: once against the real module under Hypothesis, and once, as a negative
control, against a deliberately broken variant that the same property must
reject. A property that passes a broken interpreter proves nothing.

The runs are deterministic (``derandomize``) and bounded, with no example
database, so they cannot flake and cost well under a second each.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import itertools
import typing as typ
from collections.abc import Callable

import pytest
import workflow_legs
from hypothesis import given, settings
from hypothesis import strategies as st

SETTINGS: typ.Final[settings] = settings(
    max_examples=100, derandomize=True, database=None, deadline=None
)

#: Matrix keys and values drawn from a small alphabet: the content is not the
#: subject, collisions between draws are what exercises the interpreter.
_KEY = st.text(alphabet="abcd", min_size=1, max_size=2)
_VALUE = st.text(alphabet="xyz01", min_size=1, max_size=3)

type LegsImpl = Callable[[str, str, dict[str, object]], list[workflow_legs.Leg]]


@st.composite
def axes_strategy(draw: st.DrawFn) -> dict[str, list[str]]:
    """Draw one to three axes, each with one to three distinct values."""
    keys = draw(st.lists(_KEY, min_size=1, max_size=3, unique=True))
    return {key: draw(st.lists(_VALUE, min_size=1, max_size=3, unique=True)) for key in keys}


def product_of(axes: dict[str, list[str]]) -> list[dict[str, str]]:
    """Return the Cartesian product the slow, obvious way, as the oracle."""
    cells: list[dict[str, str]] = [{}]
    for key, values in axes.items():
        cells = [{**cell, key: value} for cell in cells for value in values]
    return cells


def job_with(matrix: dict[str, object] | None, **extra: object) -> dict[str, object]:
    """Return a job mapping, with a matrix when one is given."""
    job: dict[str, object] = dict(extra)
    if matrix is not None:
        job["strategy"] = {"matrix": matrix}
    return job


def cells_of(impl: LegsImpl, job: dict[str, object]) -> list[dict[str, str]]:
    """Return the cells *impl* produces for *job*."""
    return [leg.cell for leg in impl("ci.yml", "build", job)]


# -- legs ----------------------------------------------------------------------------


def check_product_is_complete(impl: LegsImpl, axes: dict[str, list[str]]) -> None:
    """A matrix of list axes yields exactly their product: every cell once, none extra."""
    cells = cells_of(impl, job_with(dict(axes)))
    expected = product_of(axes)
    assert len(cells) == len(expected) == len({tuple(sorted(c.items())) for c in cells}), (
        "matrix legs should contain each product cell exactly once"
    )
    assert sorted(map(sorted, (c.items() for c in cells))) == sorted(
        map(sorted, (c.items() for c in expected))
    ), f"matrix legs differ from the Cartesian product: {cells!r} != {expected!r}"


def check_include_follows_survivors(
    impl: LegsImpl, axes: dict[str, list[str]], extra: list[dict[str, str]]
) -> None:
    """Includes are appended after the product, in order, unchanged."""
    cells = cells_of(impl, job_with({**axes, "include": list(extra)}))
    assert cells[len(cells) - len(extra) :] == extra, (
        "matrix includes should follow the product unchanged"
    )
    assert len(cells) == len(product_of(axes)) + len(extra), (
        "matrix includes should append one leg each"
    )


def check_exclude_removes_exactly_the_matches(
    impl: LegsImpl, axes: dict[str, list[str]], pick: int, keep: list[bool]
) -> None:
    """An exclusion, even a partial one, removes every cell it matches and no other."""
    product = product_of(axes)
    victim = product[pick % len(product)]
    keys = [key for key, flag in zip(victim, keep, strict=False) if flag] or [next(iter(victim))]
    exclusion = {key: victim[key] for key in keys}
    cells = cells_of(impl, job_with({**axes, "exclude": [exclusion]}))
    survivors = [c for c in product if not all(c[k] == v for k, v in exclusion.items())]
    # A matrix that excludes every cell is read as one empty leg, the same
    # over-approximation as a job with no matrix: the contract would rather
    # demand an installer of a job that cannot run than excuse one that might.
    assert cells == (survivors or [{}]), f"exclude should remove only matching cells: {cells!r}"
    assert victim not in cells or not survivors, (
        "an excluded victim should not survive beside other cells"
    )


def check_a_job_without_cells_is_one_empty_leg(impl: LegsImpl, matrix: object) -> None:
    """No matrix, or one that names no cell, still runs once with an empty cell."""
    job = job_with(None) if matrix is None else {"strategy": {"matrix": matrix}}
    assert cells_of(impl, job) == [{}], (
        "a job without runnable matrix cells should have one empty leg"
    )


# -- runner labels -------------------------------------------------------------------


def leg_with(runs_on: object, cell: dict[str, str] | None = None) -> workflow_legs.Leg:
    """Return a leg whose job declares *runs_on* and whose matrix cell is *cell*."""
    return workflow_legs.Leg("ci.yml", "build", cell or {}, {"runs-on": runs_on})


def check_plain_forms_resolve_to_their_labels(labels: list[str], group: str) -> None:
    """A string, a list and a ``labels`` mapping resolve to their labels, group last."""
    assert workflow_legs.runner_labels(leg_with(labels[0])) == [labels[0]], (
        "a scalar runner label should resolve"
    )
    assert workflow_legs.runner_labels(leg_with(list(labels))) == labels, (
        "runner label lists should retain order"
    )
    assert workflow_legs.runner_labels(leg_with({"labels": labels[0]})) == [
        labels[0]
    ], "a labels mapping should resolve its scalar label"
    mapping = {"labels": list(labels), "group": group}
    assert workflow_legs.runner_labels(leg_with(mapping)) == [*labels, group], (
        "runner groups should follow labels"
    )


def check_matrix_references_are_substituted(cell: dict[str, str], literal: str) -> None:
    """Each ``matrix.<key>`` becomes the leg's value; a fork fallback names both arms."""
    first, second = (list(cell) * 2)[:2]
    fallback = f"${{{{ github.event.fork && matrix.{first} || matrix.{second} }}}}"
    assert workflow_legs.runner_labels(leg_with(fallback, cell)) == [
        cell[first],
        cell[second],
    ], "both readable arms of a fork fallback should resolve"
    quoted = f"${{{{ matrix.{first} && '{literal}' }}}}"
    assert workflow_legs.runner_labels(leg_with(quoted, cell)) == [
        cell[first],
        literal,
    ], "matrix references and string literals should both resolve"


def check_unreadable_runners_raise(key: str) -> None:
    """An absent matrix key, an unknowable source and a non-form each refuse a verdict."""
    for runs_on in (
        f"${{{{ matrix.{key} }}}}",
        f"${{{{ inputs.{key} }}}}",
        f"${{{{ github.{key} }}}}",
        None,
        7,
    ):
        with pytest.raises(workflow_legs.UnreadableRunnerError):
            workflow_legs.runner_labels(leg_with(runs_on))
    # A readable arm does not excuse an unknowable one: the leg may land on
    # whatever `inputs` names, so no Linux verdict can be drawn from the rest.
    mixed = f"${{{{ matrix.{key} || inputs.{key} }}}}"
    with pytest.raises(workflow_legs.UnreadableRunnerError):
        workflow_legs.runner_labels(leg_with(mixed, {key: "x"}))


# -- step conditions -----------------------------------------------------------------


def check_conditions_agree_with_an_oracle(
    cell: dict[str, str], tests: list[tuple[str, str, bool]]
) -> None:
    """A conjunction of matrix comparisons holds exactly when every comparison does."""
    clauses, expected = [], True
    for key, value, equal in tests:
        clauses.append(f"matrix.{key} {'==' if equal else '!='} '{value}'")
        actual = cell.get(key)
        expected = expected and (actual is None or (actual == value) == equal)
    step = {"if": "${{ " + " && ".join(clauses) + " }}"}
    assert workflow_legs.step_runs_on(step, cell) is expected, (
        f"matrix condition result differs from its oracle for {cell!r} and {tests!r}"
    )


def check_unevaluable_conditions_run(cell: dict[str, str], key: str, value: str) -> None:
    """No condition, an ``||``, or a non-matrix test never excuses a step."""
    assert workflow_legs.step_runs_on({}, cell), (
        "an absent condition should leave the step enabled"
    )
    assert workflow_legs.step_runs_on(
        {"if": f"matrix.{key} == '{value}' || always()"}, cell
    ), "an unevaluable disjunction should conservatively leave the step enabled"
    assert workflow_legs.step_runs_on({"if": "${{ always() }}"}, cell), (
        "an unevaluable function should conservatively leave the step enabled"
    )


# -- the real interpreter under Hypothesis -------------------------------------------


@SETTINGS
@given(axes_strategy())
def test_the_product_is_complete(axes: dict[str, list[str]]) -> None:
    """Every product cell appears exactly once, whatever the axes."""
    check_product_is_complete(workflow_legs.legs, axes)


@SETTINGS
@given(axes_strategy(), st.lists(st.dictionaries(_KEY, _VALUE, min_size=1, max_size=3), max_size=3))
def test_includes_follow_the_product(axes: dict[str, list[str]], extra: list[dict[str, str]]) -> None:
    """Includes are appended unchanged after the product."""
    check_include_follows_survivors(workflow_legs.legs, axes, extra)


@SETTINGS
@given(axes_strategy(), st.integers(min_value=0), st.lists(st.booleans(), min_size=1, max_size=3))
def test_an_exclusion_removes_exactly_what_it_matches(
    axes: dict[str, list[str]], pick: int, keep: list[bool]
) -> None:
    """A full or partial exclusion removes its matches and nothing else."""
    check_exclude_removes_exactly_the_matches(workflow_legs.legs, axes, pick, keep)


@pytest.mark.parametrize("matrix", [None, {}, {"include": []}, {"os": "not-a-list"}])
def test_a_job_without_cells_is_one_empty_leg(matrix: object) -> None:
    """The degenerate shapes still produce exactly one leg with an empty cell."""
    check_a_job_without_cells_is_one_empty_leg(workflow_legs.legs, matrix)


@SETTINGS
@given(st.lists(_VALUE, min_size=1, max_size=3), _VALUE)
def test_plain_runner_forms_resolve(labels: list[str], group: str) -> None:
    """Scalar, list and mapping forms resolve to their labels."""
    check_plain_forms_resolve_to_their_labels(labels, group)


@SETTINGS
@given(st.dictionaries(_KEY, _VALUE, min_size=1, max_size=3), _VALUE)
def test_matrix_references_are_substituted(cell: dict[str, str], literal: str) -> None:
    """Matrix references become the leg's values, including both fork-fallback arms."""
    check_matrix_references_are_substituted(cell, literal)


@SETTINGS
@given(_KEY)
def test_unreadable_runners_raise(key: str) -> None:
    """A runner the text cannot name is refused, never read as non-Linux."""
    check_unreadable_runners_raise(key)


@SETTINGS
@given(
    st.dictionaries(_KEY, _VALUE, max_size=3),
    st.lists(st.tuples(_KEY, _VALUE, st.booleans()), min_size=1, max_size=3),
)
def test_conditions_agree_with_the_oracle(
    cell: dict[str, str], tests: list[tuple[str, str, bool]]
) -> None:
    """Matrix comparisons are evaluated, conjoined, and unknown keys hold."""
    check_conditions_agree_with_an_oracle(cell, tests)


@SETTINGS
@given(st.dictionaries(_KEY, _VALUE, max_size=3), _KEY, _VALUE)
def test_unevaluable_conditions_never_excuse_a_step(
    cell: dict[str, str], key: str, value: str
) -> None:
    """The safe reading: what cannot be evaluated is treated as running."""
    check_unevaluable_conditions_run(cell, key, value)


# -- negative controls: the same properties must reject a wrong interpreter ---------


def _drops_the_last_axis(workflow: str, job_id: str, job: dict[str, object]) -> list[workflow_legs.Leg]:
    """A wrong ``legs``: the product forgets its final axis."""
    matrix = dict(job["strategy"]["matrix"])  # type: ignore[index]
    matrix.pop(next(reversed(matrix)))
    return workflow_legs.legs(workflow, job_id, job_with(matrix or None))


def _ignores_exclude(workflow: str, job_id: str, job: dict[str, object]) -> list[workflow_legs.Leg]:
    """A wrong ``legs``: ``exclude`` is read as nothing."""
    matrix = {k: v for k, v in job["strategy"]["matrix"].items() if k != "exclude"}  # type: ignore[index]
    return workflow_legs.legs(workflow, job_id, job_with(matrix))


def _rejects(check: Callable[..., None], *strategies: st.SearchStrategy[typ.Any]) -> None:
    """Assert that Hypothesis finds an input on which *check* fails."""

    # One failure is enough to reject; reporting several would wrap them in an
    # ExceptionGroup, which is not the AssertionError the control expects.
    @settings(SETTINGS, report_multiple_bugs=False)
    @given(st.tuples(*strategies))
    def run(args: tuple[object, ...]) -> None:
        """Run the property check with Hypothesis-generated arguments."""
        check(*args)

    with pytest.raises(AssertionError):
        run()


def test_the_product_property_rejects_a_dropped_axis() -> None:
    """Control: an interpreter that loses an axis fails completeness."""
    _rejects(lambda axes: check_product_is_complete(_drops_the_last_axis, axes), axes_strategy())


def test_the_exclusion_property_rejects_an_ignored_exclude() -> None:
    """Control: an interpreter that ignores ``exclude`` fails the exclusion property."""
    _rejects(
        lambda axes, pick, keep: check_exclude_removes_exactly_the_matches(
            _ignores_exclude, axes, pick, keep
        ),
        axes_strategy(),
        st.integers(min_value=0),
        st.lists(st.booleans(), min_size=1, max_size=3),
    )


def test_the_condition_property_rejects_a_swapped_operator(monkeypatch: pytest.MonkeyPatch) -> None:
    """Control: ``==`` and ``!=`` swapped in the comparison fails the oracle."""
    monkeypatch.setattr(
        workflow_legs, "_comparison_holds", lambda actual, operator, value: True
    )
    _rejects(
        check_conditions_agree_with_an_oracle,
        st.dictionaries(_KEY, _VALUE, min_size=1, max_size=3),
        st.lists(st.tuples(_KEY, _VALUE, st.booleans()), min_size=1, max_size=3),
    )


def test_the_substitution_property_rejects_an_unsubstituted_reference(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Control: an interpreter that returns the raw expression fails substitution."""
    monkeypatch.setattr(workflow_legs, "_resolve_label", lambda label, leg: [label])
    _rejects(
        check_matrix_references_are_substituted,
        st.dictionaries(_KEY, _VALUE, min_size=1, max_size=3),
        _VALUE,
    )


def test_the_product_oracle_is_itself_the_itertools_product() -> None:
    """The slow oracle agrees with ``itertools.product`` on a fixed example."""
    axes = {"a": ["x", "y"], "b": ["0", "1", "z"]}
    expected = [dict(zip(axes, combo, strict=True)) for combo in itertools.product(*axes.values())]
    assert product_of(axes) == expected
