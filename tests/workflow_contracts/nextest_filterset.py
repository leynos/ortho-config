"""Reading a nextest filterset expression as the runner reads it.

An allowance is attached to a ``filter``, so deciding whether one covers a
binary means deciding whether the expression selects it. That is a reading
of nextest's own filter language, and it is kept apart from
:mod:`trybuild_tier` -- which knows the class but not the language -- so
each stays legible and neither outgrows the 400-line limit ``AGENTS.md``
sets.

Only disjunctions of ``binary(...)`` and exact ``binary_id(package::target)``
predicates are read. A conjunction, a difference, or a negation narrows the
selection in a way that rating its terms alone would report as wider than it
is, so such an expression is refused rather than misread. Refusing is the safe
direction throughout: an expression read as matching nothing would report the
whole class as uncovered, and one read as matching everything would report a
broken filter as sound.
"""

from __future__ import annotations

import fnmatch
import re
import typing as typ


class UnreadableMatcherError(Exception):
    """Raised when a filter term is outside the matchers this reads.

    This reader deliberately supports fewer forms than nextest's full
    filter language. Refusing an unfamiliar form is safer than reading it as
    matching nothing or everything and silently certifying a broken filter.
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
    if not matcher:
        raise UnreadableMatcherError("an empty binary matcher is not supported")
    match matcher[:1], matcher:
        case ("=", _):
            return matcher[1:] == name
        case ("~", _):
            return matcher[1:] in name
        case ("/", _):
            pattern = matcher[1:-1] if matcher.endswith("/") else None
            if pattern is None:
                raise UnreadableMatcherError(f"unterminated regex matcher {matcher!r}")
            try:
                return re.search(pattern, name) is not None
            except re.error as error:
                raise UnreadableMatcherError(
                    f"invalid regex matcher {matcher!r}: {error}"
                ) from error
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


def _matches_binary_id(matcher: str, name: str) -> bool:
    """Compare an exact ``package::target`` ID with its target name.

    The caller supplies workspace-wide target names rather than package IDs,
    so the package component is validated but cannot narrow the candidate set.
    This deliberate over-approximation can flag an unrelated target with the
    same name; it cannot hide a target that the ID selects.

    Examples
    --------
    >>> _matches_binary_id("ortho_config::compile_fail", "compile_fail")
    True
    >>> _matches_binary_id("ortho_config::compile_fail", "cli_dispatch")
    False
    """
    parts = matcher.split("::")
    if len(parts) != 2 or not all(
        re.fullmatch(r"[A-Za-z0-9_][A-Za-z0-9_.-]*", part) for part in parts
    ):
        raise UnreadableMatcherError(
            f"binary_id matcher {matcher!r} is not an exact package::target ID"
        )
    _, target = parts
    return target == name


def _matches_term(kind: str, matcher: str, name: str) -> bool:
    """Apply one supported filter term to a candidate target name.

    Examples
    --------
    >>> _matches_term("binary_id", "ortho_config::compile_fail", "compile_fail")
    True
    >>> _matches_term("binary", "cli*", "cli_dispatch")
    True
    """
    if kind == "binary":
        return _matches_one(matcher, name)
    return _matches_binary_id(matcher, name)


FILTER_TERM = re.compile(r"(?P<kind>binary_id|binary)\((?P<matcher>[^)]*)\)")


def binaries_selected_by(
    filter_expression: str, candidates: typ.Iterable[str]
) -> dict[str, bool]:
    """Return which candidates a disjunction of supported terms selects.

    Only a ``|``-joined list of ``binary`` or exact ``binary_id`` predicates
    is read. A conjunction, difference, negation, or other term narrows the
    selection in a way this reader does not evaluate, so the whole expression
    is refused rather than treating one term as the result.

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
        If the expression is not a disjunction of supported terms, names no
        supported predicate, or uses a matcher this reader cannot evaluate.

    Examples
    --------
    >>> binaries_selected_by('binary(*trybuild)', ["a_trybuild", "other"])
    {'a_trybuild': True, 'other': False}
    >>> binaries_selected_by('binary_id(pkg::compile_fail)', ["compile_fail", "other"])
    {'compile_fail': True, 'other': False}
    """
    terms = _supported_terms(filter_expression)
    return {
        name: any(_matches_term(kind, matcher, name) for kind, matcher in terms)
        for name in candidates
    }


def _supported_terms(filter_expression: str) -> list[tuple[str, str]]:
    """Return the trimmed terms of a single disjunction, refusing anything else.

    Each matcher is validated here rather than per candidate, so an
    unevaluable matcher fails once instead of once per binary name.

    Examples
    --------
    >>> _supported_terms("binary(a) | binary_id(pkg::b)")
    [('binary', 'a'), ('binary_id', 'pkg::b')]
    """
    matches = list(FILTER_TERM.finditer(filter_expression))
    _refuse_unsupported_terms(filter_expression, matches)
    terms = [(match.group("kind"), match.group("matcher").strip()) for match in matches]
    for kind, matcher in terms:
        _matches_term(kind, matcher, "")
    return terms


def _refuse_unsupported_terms(
    filter_expression: str, matches: list[re.Match[str]]
) -> None:
    """Raise unless the expression is only supported terms joined by ``|``.

    Examples
    --------
    >>> _refuse_unsupported_terms("binary(a)", [])
    Traceback (most recent call last):
    ...
    nextest_filterset.UnreadableMatcherError: the filter has no supported term: 'binary(a)'
    """
    if not matches:
        raise UnreadableMatcherError(
            f"the filter has no supported term: {filter_expression!r}"
        )
    if _is_single_disjunction(filter_expression, matches):
        return
    raise UnreadableMatcherError(
        f"the filter combines its supported terms with something other "
        f"than a single `|`, which this reader does not evaluate; "
        f"reading the terms alone could report a narrower expression as "
        f"wider than it is: "
        f"{filter_expression!r}"
    )


def _is_single_disjunction(
    filter_expression: str, matches: list[re.Match[str]]
) -> bool:
    """Report whether the matched terms are separated by a single ``|`` each.

    ``matches`` must be non-empty; the caller refuses an expression with no
    supported term before asking this.

    Examples
    --------
    >>> _is_single_disjunction("binary(a) | binary(b)", list(FILTER_TERM.finditer("binary(a) | binary(b)")))
    True
    >>> _is_single_disjunction("binary(a) & binary(b)", list(FILTER_TERM.finditer("binary(a) & binary(b)")))
    False
    """
    separators = [
        filter_expression[left.end() : right.start()].strip()
        for left, right in zip(matches, matches[1:])
    ]
    leading = filter_expression[: matches[0].start()].strip()
    trailing = filter_expression[matches[-1].end() :].strip()
    has_outside = bool(leading) or bool(trailing)
    return not has_outside and all(separator == "|" for separator in separators)
