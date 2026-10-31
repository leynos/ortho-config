"""Contract for the ``boundary`` Make target.

The target is the focused entry point for the standalone-runtime boundary
contracts: an application depending on ``ortho_l10n`` alone must not resolve
the configuration facade, the derive macros, Figment, or Clap. Two properties
of the target are load-bearing, and both are invisible if only its name is
checked.

The first is that it runs *only* the boundary target of ``ortho_l10n``.
``make boundary`` that also ran the workspace would turn a boundary failure
into one line among thousands, and it would re-run a suite ``make test``
already covers. The second is that it runs offline, because the fixtures
resolve foreign package roots from the shared package cache on purpose: a
resolve that silently reached crates.io would prove something about the
network rather than about the shipped graph.

A third property is deliberately *not* asserted as a whole: the recipe must
not claim a cold measurement. The fixtures keep ``target/localization-boundary``
between runs, so a run is warm by design and the comment says so. Nothing here
executes the suite — it takes minutes and builds foreign crates, which belongs
to the gate runner and not to a contract test.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import shlex
import typing as typ

import pytest
from makefile_support import recipe_lines

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
MAKEFILE_PATH: typ.Final[pathlib.Path] = REPOSITORY_ROOT / "Makefile"

#: The target under contract.
TARGET: typ.Final[str] = "boundary"

#: The package and test target the recipe must select.
PACKAGE: typ.Final[str] = "ortho_l10n"
TEST_TARGET: typ.Final[str] = "boundary"

#: Flags the recipe must not carry: each widens or redirects the run.
FORBIDDEN_FLAGS: typ.Final[dict[str, str]] = {
    "--workspace": "the target must select one package, not the workspace",
    "--all-targets": "the target must select one test target, not every target",
    "--all-features": "the boundary claim is about the default graph a stranger resolves",
}


def _recipe() -> list[str]:
    """Return the recipe lines of the target under contract."""
    return recipe_lines(MAKEFILE_PATH.read_text(encoding="utf-8"), TARGET)


def _invocation() -> list[str]:
    """Return the target's recipe as one token list.

    A recipe may spread its command over continuation lines, so the lines are
    joined before they are split. ``RUSTFLAGS="-D warnings"`` is a prefix
    assignment rather than part of the command, and ``shlex`` keeps it as one
    token, which the assertions below ignore.
    """
    joined = " ".join(line.removesuffix("\\").strip() for line in _recipe())
    return shlex.split(joined, comments=True)


def test_the_target_exists() -> None:
    """A target that is only documented in ``.PHONY`` runs nothing.

    Scenario: ``make boundary`` is the entry point agents and the developers'
    guide name. Invariant: the Makefile defines a recipe for it, so the
    documented command is not a no-op.
    """
    assert _recipe(), f"the Makefile defines no recipe for {TARGET}"


def test_the_target_selects_the_boundary_test_of_ortho_l10n() -> None:
    """The selection is what makes a failure attributable.

    Scenario: the boundary suite fails. Invariant: the recipe selects the
    ``ortho_l10n`` package and the ``boundary`` test target by name, so the
    failure is reported against one binary rather than the workspace and the
    run does not duplicate ``make test``.
    """
    tokens = _invocation()
    assert PACKAGE in _flag_values("-p"), (
        f"the recipe does not select -p {PACKAGE}: {tokens}"
    )
    assert TEST_TARGET in _flag_values("--test"), (
        f"the recipe does not select --test {TEST_TARGET}: {tokens}"
    )


def test_the_target_does_not_widen_the_selection() -> None:
    """A wider selection would hide which contract failed.

    Scenario: someone adds ``--workspace`` or ``--all-targets`` to save a
    command. Invariant: neither flag is present, because both would re-run
    suites this target exists to isolate.
    """
    tokens = _invocation()
    present = sorted(flag for flag in FORBIDDEN_FLAGS if flag in tokens)
    assert not present, "; ".join(
        f"{flag} is present: {FORBIDDEN_FLAGS[flag]}" for flag in present
    )


@pytest.mark.parametrize("flag", ["--offline"])
def test_the_target_resolves_from_the_shared_package_cache(flag: str) -> None:
    """Offline is evidence, not an optimization.

    Scenario: the boundary contracts resolve from the local cache. Invariant:
    the recipe passes the flag, so a run that reached the network cannot be
    mistaken for a run that proved the shipped graph.
    """
    assert flag in _invocation(), f"the recipe does not pass {flag}"


def test_the_target_denies_warnings() -> None:
    """The workspace denies warnings; a focused run must not be laxer.

    Scenario: the boundary fixtures compile with a warning. Invariant: the
    recipe sets ``RUSTFLAGS`` to deny warnings, as ``make test`` does, so the
    focused entry point cannot pass where the full gate would fail.

    The assignment is read as a token rather than as a substring: a recipe
    line is one string, so a substring test would also accept the flag inside
    a comment or a quoted argument where it sets nothing.
    """
    tokens = _invocation()
    # The development build standard adds the linker flag after the warning
    # policy, so the token is the policy plus ``$(STANDARD_RUSTFLAGS)``.
    assert "RUSTFLAGS=-D warnings $(STANDARD_RUSTFLAGS)" in tokens, (
        f"the boundary recipe must deny warnings the way `make test` does: {tokens}"
    )


def _flag_values(flag: str) -> list[str]:
    """Return the values passed to ``flag`` in the recipe.

    Parameters
    ----------
    flag : str
        The flag whose following token is wanted, such as ``--test``.

    Returns
    -------
    list of str
        One value per occurrence, empty when the flag is absent.
    """
    tokens = _invocation()
    return [
        tokens[index + 1]
        for index, token in enumerate(tokens)
        if token == flag and index + 1 < len(tokens)
    ]
