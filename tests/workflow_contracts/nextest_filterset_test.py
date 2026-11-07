"""Regression cases for filter forms the precedence reader accepts.

The reader supports exact ``binary_id`` values from the Windows trybuild
override while refusing filters whose selected binaries it cannot determine.
"""

from __future__ import annotations

import pytest

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
