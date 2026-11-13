"""Regression cases for filter forms the precedence reader accepts.

The reader supports exact ``binary_id`` values from the Windows trybuild
override while refusing filters whose selected binaries it cannot determine.
The named cases pin the shapes that failed; the generated properties below
vary the disjunction, the candidates and the separator, and compare the
selection with a model that never asks the reader to read anything.
"""

from __future__ import annotations

import typing as typ

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

from nextest_filterset import UnreadableMatcherError, binaries_selected_by


def test_exact_binary_ids_select_their_workspace_target_names() -> None:
    """An exact package and target ID selects its bare target name."""
    selected = binaries_selected_by(
        "binary_id(ortho_config::compile_fail)"
        " | binary_id(cargo-orthohelp::compile_time)",
        ["compile_fail", "compile_time", "other"],
    )
    assert selected == {
        "compile_fail": True,
        "compile_time": True,
        "other": False,
    }


def test_an_exact_binary_id_can_share_a_disjunction_with_binary() -> None:
    """Supported terms compose only through the disjunction operator."""
    selected = binaries_selected_by(
        "binary(cli_dispatch) | binary_id(ortho_config::compile_fail)",
        ["cli_dispatch", "compile_fail", "other"],
    )
    assert selected == {
        "cli_dispatch": True,
        "compile_fail": True,
        "other": False,
    }


@pytest.mark.parametrize(
    "expression",
    [
        "binary_id(ortho_config::*)",
        "binary_id(~compile_fail)",
        "binary_id(ortho_config::compile_fail) & test(slow)",
        "binary(a) & test(slow)",
        "binary(a)binary(b)",
        "binary(a)||binary(b)",
        "binary(a) |",
        "binary()",
        "test(slow)",
        "package(ortho_config)",
        "all()",
        "binary(/[invalid/)",
    ],
)
def test_unsupported_or_unreadable_filters_are_refused(expression: str) -> None:
    """The reader refuses any filter shape it cannot evaluate safely."""
    with pytest.raises(UnreadableMatcherError):
        binaries_selected_by(expression, ["compile_fail", "cli_dispatch"])


def test_unreadable_terms_are_validated_without_candidates() -> None:
    """An empty candidate set cannot turn an unreadable filter into a pass."""
    with pytest.raises(UnreadableMatcherError):
        binaries_selected_by("binary_id(package::*)", [])


#: The names every generated expression and candidate is drawn from. The
#: alphabet holds no glob metacharacter, so a bare ``binary(name)`` term names
#: exactly one binary and the model can compare strings directly rather than
#: reimplementing ``fnmatch``. It also holds no ``-``, because a ``binary_id``
#: component must begin with a letter, digit or underscore and a leading ``-``
#: would make the generated term unreadable for a reason this property is not
#: testing.
_NAME_ALPHABET: typ.Final[str] = "abc_"
_NAMES = st.text(alphabet=_NAME_ALPHABET, min_size=1, max_size=6)

#: Separators that are not the single ``|`` the reader accepts. Each one is a
#: shape nextest would evaluate as a conjunction, a negation or a syntax
#: error, so reading its terms alone could report a narrower expression as
#: wider.
_INVALID_SEPARATORS: typ.Final[tuple[str, ...]] = (
    "",
    " ",
    " & ",
    "&",
    " || ",
    "||",
    " and ",
    " ^ ",
)


@st.composite
def _exact_disjunctions(draw: st.DrawFn) -> tuple[str, list[tuple[str, str]]]:
    """Draw a disjunction of exact terms with the names those terms select.

    The returned pairs are the truth the model compares against: each holds
    the kind and the target name the term names, so the model never re-reads
    the expression the reader under test is asked to read.
    """
    names = draw(st.lists(_NAMES, min_size=1, max_size=4, unique=True))
    packages = draw(st.lists(_NAMES, min_size=len(names), max_size=len(names)))
    terms: list[tuple[str, str]] = []
    pieces: list[str] = []
    for name, package in zip(names, packages):
        if draw(st.booleans()):
            terms.append(("binary_id", name))
            pieces.append(f"binary_id({package}::{name})")
        else:
            terms.append(("binary", name))
            pieces.append(f"binary({name})")
    return draw(st.sampled_from([" | ", "|", "  |  "])).join(pieces), terms


@settings(max_examples=100, deadline=None)
@given(
    disjunction=_exact_disjunctions(),
    candidates=st.lists(_NAMES, min_size=1, max_size=5),
)
def test_selection_is_the_union_of_its_terms(
    disjunction: tuple[str, list[tuple[str, str]]], candidates: list[str]
) -> None:
    """A candidate is selected exactly when some term names it.

    The model is a union over the drawn terms rather than a second parse, so
    a reader that dropped a term, read one term's name as another's, or
    treated the disjunction as a conjunction disagrees with it.
    """
    expression, terms = disjunction
    expected = {
        candidate: any(candidate == name for _, name in terms)
        for candidate in candidates
    }
    assert binaries_selected_by(expression, candidates) == expected, (
        f"{expression!r} over {candidates} did not select the union of {terms}"
    )


@settings(max_examples=100, deadline=None)
@given(
    names=st.lists(_NAMES, min_size=2, max_size=4, unique=True),
    separator=st.sampled_from(_INVALID_SEPARATORS),
)
def test_terms_joined_by_anything_but_a_single_pipe_are_refused(
    names: list[str], separator: str
) -> None:
    """Every separator but ``|`` is refused, however the terms are spelled."""
    expression = separator.join(f"binary({name})" for name in names)
    with pytest.raises(UnreadableMatcherError):
        binaries_selected_by(expression, names)


@settings(max_examples=50, deadline=None)
@given(name=_NAMES)
def test_a_binary_id_term_without_a_package_is_refused(name: str) -> None:
    """A ``binary_id`` term needs both halves of its ``package::target`` ID."""
    with pytest.raises(UnreadableMatcherError):
        binaries_selected_by(f"binary_id({name})", [name])
