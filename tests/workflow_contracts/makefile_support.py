"""Parsing helpers for Makefile recipes, shared by the workflow contracts.

A contract that names a Make target proves only half of what it claims: the
target has to do the thing as well. These helpers read a recipe so a contract
can assert both ends. They live apart from any one contract because more than
one needs them, and because their own examples are executed by the
``--doctest-modules`` the contract gate passes.
"""

from __future__ import annotations

import itertools
import re
import shlex


def recipe_lines(makefile: str, target: str) -> list[str]:
    r"""Return the recipe lines of one Make target.

    Parameters
    ----------
    makefile : str
        The text of a Makefile.
    target : str
        The target to read.

    Returns
    -------
    list of str
        The target's tab-indented recipe lines, empty when no rule defines
        that target.

    Examples
    --------
    >>> recipe_lines("all:\n\techo hi\n", "all")
    ['echo hi']
    >>> recipe_lines("all:\n\techo hi\n", "absent")
    []
    """
    rule = re.compile(rf"^{re.escape(target)}\s*:(?!=)", re.MULTILINE)
    match = rule.search(makefile)
    if match is None:
        return []
    body = makefile[match.end() :].splitlines()[1:]
    return [line[1:] for line in itertools.takewhile(is_recipe_line, body)]


def is_recipe_line(line: str) -> bool:
    r"""Report whether a line belongs to the recipe currently being read.

    Parameters
    ----------
    line : str
        One line of a Makefile, taken in the order they appear.

    Returns
    -------
    bool
        True for a tab-indented line, which Make treats as a recipe line.

    Examples
    --------
    >>> is_recipe_line("\techo hi"), is_recipe_line("other:")
    (True, False)
    """
    return line.startswith("\t")


def lading_subcommand(makefile: str, target: str) -> str | None:
    r"""Return the lading subcommand a Make target's recipe invokes.

    The recipe names lading through a variable, so the token is matched by
    suffix: `$(LADING)` expands to a `uvx --from ... lading` invocation.

    Parameters
    ----------
    makefile : str
        The text of a Makefile.
    target : str
        The target whose recipe is read.

    Returns
    -------
    str or None
        The first token after the one naming lading, or ``None`` when the
        recipe runs no lading command or names no subcommand.

    Examples
    --------
    >>> lading_subcommand("publish-check:\n\t$(LADING) publish .\n",
    ...                   "publish-check")
    'publish'
    >>> lading_subcommand("publish-check:\n\techo nothing\n",
    ...                   "publish-check") is None
    True
    """
    for line in recipe_lines(makefile, target):
        tokens = shlex.split(line, comments=True)
        named = next(
            (
                index
                for index, token in enumerate(tokens)
                if token == "lading" or token.upper().endswith("LADING)")
            ),
            None,
        )
        if named is None:
            continue
        following = tokens[named + 1 :]
        return following[0] if following else None
    return None
