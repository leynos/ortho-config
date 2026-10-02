"""Reading a nextest filterset expression as the runner reads it.

An allowance is attached to a ``filter``, so deciding whether one covers a
binary means deciding whether the expression selects it. That is a reading
of nextest's own filter language, and it is kept apart from
:mod:`trybuild_tier` -- which knows the class but not the language -- so
each stays legible and neither outgrows the 400-line limit ``AGENTS.md``
sets.

Only the shape the trybuild override uses is read: a ``|``-joined list of
``binary(...)`` predicates. A conjunction, a difference, or a negation
narrows the selection in a way that rating the ``binary`` terms alone
would report as wider than it is, so such an expression is refused rather
than misread. Refusing is the safe direction throughout: an expression
read as matching nothing would report the whole class as uncovered, and
one read as matching everything would report a broken filter as sound.
"""

from __future__ import annotations

import fnmatch
import re
import typing as typ


class UnreadableMatcherError(Exception):
    """Raised when a ``binary(...)`` argument is not a matcher this reads.

    Every matcher in the nextest reference is read here, but a form added
    later would not be. Refusing is the safe direction: a matcher read as
    matching nothing would report the whole trybuild class as uncovered,
    and one read as matching everything would report a broken filter as
    sound. Neither is a guess worth making silently.
    """


def _matches_one(matcher: str, name: str) -> bool:
    """Report whether one ``binary(...)`` argument matches a binary name.

    The matchers are the ones in the nextest filterset reference. A bare
    argument, and a ``#``-prefixed one, are globs -- that is nextest's
    default for ``binary()``, so `binary(foo)` is an exact match only
    because `foo` holds no metacharacter.

    Parameters
    ----------
    matcher : str
        The argument of one ``binary(...)`` call, trimmed.
    name : str
        A binary name.

    Raises
    ------
    UnreadableMatcherError
        If the argument is not a matcher this contract reads.

    Returns
    -------
    bool
        Whether this argument selects that name.

    Examples
    --------
    >>> _matches_one("compile_fail", "compile_fail")
    True
    >>> _matches_one("*trybuild", "crate_path_trybuild")
    True
    >>> _matches_one("=compile_fail", "compile_fail")
    True
    >>> _matches_one("~fail", "compile_fail")
    True
    >>> _matches_one("/^compile_/", "compile_fail")
    True
    """
    match matcher[:1], matcher:
        case ("=", _):
            return matcher[1:] == name
        case ("~", _):
            return matcher[1:] in name
        case ("/", _):
            pattern = matcher[1:-1] if matcher.endswith("/") else None
            if pattern is None:
                raise UnreadableMatcherError(f"unterminated regex matcher {matcher!r}")
            return re.search(pattern, name) is not None
        case ("#", _):
            pattern = matcher[1:]
        case _:
            pattern = matcher
    # `globset` reads `{a,b}` alternation and `fnmatch` does not, so the
    # difference is refused rather than left to disagree with nextest.
    if "{" in pattern:
        raise UnreadableMatcherError(
            f"glob {matcher!r} uses brace alternation, which this contract "
            f"cannot evaluate as nextest does"
        )
    return fnmatch.fnmatchcase(name, pattern)


BINARY_TERM = re.compile(r"binary\(([^)]*)\)")

#: What may sit between ``binary(...)`` terms for the disjunction to be
#: read as one. Anything else — `&`, `-`, `not(...)`, or a `test(...)`
#: term — changes the selection by narrowing it, so it is refused rather
#: than dropped on the floor.
BINARY_DISJUNCTION = re.compile(r"^[\s|]*$")


def binaries_selected_by(filter_expression: str, candidates: typ.Iterable[str]) -> dict[str, bool]:
    """Return which candidates a disjunction of ``binary`` terms selects.

    Only a ``|``-joined list of ``binary(...)`` predicates is read. That
    is the shape the trybuild override uses, and it is the only shape in
    which the `binary` terms decide the answer on their own: a
    conjunction, a difference, or a negation narrows the selection in a
    way this reader does not evaluate — `binary(compile_fail) &
    test(other)` selects nothing when `other` matches no test, while
    rating the `binary` term alone would report `compile_fail` as
    covered. Such an expression is refused rather than misread.

    Parameters
    ----------
    filter_expression : str
        A nextest filterset, as a configuration's ``filter`` key holds it.
    candidates : iterable of str
        The binary names to test.

    Returns
    -------
    dict of str to bool
        Each candidate's selection, in the order given.

    Raises
    ------
    UnreadableMatcherError
        If the expression is not a ``binary`` disjunction, names no
        ``binary(...)`` predicate, or passes a ``binary(...)`` argument
        using a matcher this contract cannot evaluate.

    Examples
    --------
    >>> binaries_selected_by('binary(*trybuild)', ["a_trybuild", "other"])
    {'a_trybuild': True, 'other': False}
    """
    terms = BINARY_TERM.findall(filter_expression)
    if not terms:
        message = (
            f"the trybuild override's filter names no binary(...) predicate: "
            f"{filter_expression!r}"
        )
        raise UnreadableMatcherError(message)
    # Removing the terms leaves the glue between them. Checking it here,
    # rather than scanning the whole expression, is what keeps an
    # operator inside a matcher — a regex or glob such as `binary(/a-b/)`
    # — from being refused as a stray operator.
    remainder = BINARY_TERM.sub("", filter_expression)
    if BINARY_DISJUNCTION.match(remainder) is None:
        message = (
            f"the trybuild override's filter combines its binary(...) terms "
            f"with something other than `|`, which this contract does not "
            f"evaluate; reading the terms alone would report a narrower "
            f"expression as a wider one: {filter_expression!r}"
        )
        raise UnreadableMatcherError(message)
    return {
        name: any(_matches_one(term.strip(), name) for term in terms)
        for name in candidates
    }
