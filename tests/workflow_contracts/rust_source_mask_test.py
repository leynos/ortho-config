"""Properties of the comment and literal mask the module-graph reader uses.

``rust_source_mask`` exists so the inline-module walker can count braces
without counting the ones inside comments and literals. A reader in that role
is only useful if it never loses a byte: every mask must keep the source
length, keep every newline, and blank exactly the construct it recognised.
Those claims hold for every input rather than for the handful the
module-graph tests happen to contain, so they are asserted here over generated
sources.

The generator builds each source from plain code interleaved with comments and
literals whose exact extent it knows, and compares the mask with the same
source with only those extents blanked. That reference is built from the
generator's own knowledge of where each construct ends, so it does not re-run
the masking logic it is checking.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

from hypothesis import given, settings
from hypothesis import strategies as st
from rust_source_mask import code_mask

#: Characters that may appear in plain code around a construct. Quotes and
#: slashes are left out so a plain run cannot start a construct of its own and
#: make the generator's expectation wrong about where masking begins.
_PLAIN = st.text(alphabet="abz09 _-+=,{}();!\n", max_size=24)

#: Characters that may appear inside a construct without ending it early.
_CONTENT = st.text(alphabet="abz09 _-+=,{}();\n", max_size=12)

#: Escapes a quoted literal may carry; each is two source bytes.
_ESCAPES: typ.Final[tuple[str, ...]] = ("\\n", "\\t", "\\\\", "\\'", '\\"', "\\0")

#: A character literal's body: one bare character or one escape.
_CHARACTER_BODIES = st.one_of(
    st.text(alphabet="abz09 _-+=,", min_size=1, max_size=1),
    st.sampled_from(_ESCAPES[:-1]),
)


class Segment(typ.NamedTuple):
    """One plain run plus an optional construct, and the mask it should take.

    ``text`` is what the generator writes; ``masked`` is the same text with the
    construct's extent blanked. Concatenating the ``text`` fields gives the
    source and concatenating the ``masked`` fields gives the expected mask, so
    the comparison is between two independent strings rather than between the
    reader and itself.
    """

    text: str
    masked: str


def _blanked(construct: str) -> str:
    """Blank every character of ``construct`` except its newlines.

    Examples
    --------
    >>> _blanked("// }")
    '    '
    >>> _blanked("x\\ny")
    ' \\n '
    """
    return "".join("\n" if character == "\n" else " " for character in construct)


@st.composite
def _nested_block_comment(draw: st.DrawFn, depth: int) -> str:
    """Draw a block comment nested ``depth`` levels deep.

    Rust block comments nest, so a reader that stops at the first ``*/`` ends
    the comment early and leaves its tail counting braces it should not. Each
    level draws body text with no slash in it, so the only ``/*`` and ``*/``
    are the ones the generator writes.
    """
    body = draw(_CONTENT)
    inner = body if depth == 1 else body + draw(_nested_block_comment(depth - 1))
    return f"/*{inner}*/"


@st.composite
def _construct(draw: st.DrawFn) -> str:
    """Draw one comment or literal whose whole extent is the masked range.

    A raw string's leading delimiter is kept free of a preceding identifier
    character by the caller's separating space, so the generator never writes
    the ``myr"..."`` shape that is a name rather than a literal.
    """
    kind = draw(
        st.sampled_from(
            ("line", "block", "string", "raw_string", "character")
        )
    )
    if kind == "line":
        # A line comment runs to the end of its line, so the terminator is
        # part of the construct: without it the comment would swallow the
        # plain run and the next segment, which is not what was generated.
        return "//" + draw(_CONTENT).replace("\n", "") + "\n"
    if kind == "block":
        return draw(_nested_block_comment(draw(st.integers(min_value=1, max_value=3))))
    if kind == "raw_string":
        hashes = "#" * draw(st.integers(min_value=0, max_value=3))
        content = draw(_CONTENT).replace('"', "")
        prefix = draw(st.sampled_from(("r", "br", "cr")))
        return f'{prefix}{hashes}"{content}"{hashes}'
    if kind == "character":
        return "'" + draw(_CHARACTER_BODIES) + "'"
    parts = [draw(_CONTENT).replace('"', "").replace("\\", "")]
    parts.extend(draw(st.lists(st.sampled_from(_ESCAPES), max_size=3)))
    return '"' + "".join(parts) + '"'


@st.composite
def _segment(draw: st.DrawFn) -> Segment:
    """Draw one plain run separated from one construct by a single space."""
    plain = draw(_PLAIN)
    construct = draw(_construct())
    text = f"{plain} {construct}"
    return Segment(text, f"{plain} {_blanked(construct)}")


@settings(max_examples=200, deadline=None)
@given(segments=st.lists(_segment(), max_size=12))
def test_the_mask_blanks_exactly_the_generated_constructs(
    segments: list[Segment],
) -> None:
    """The mask equals the source with only the constructs blanked.

    This is the strong form of the reader's contract. A mask that blanked too
    little would leave a brace inside a literal for the module walker to count;
    one that blanked too much would hide a real module boundary. Both show up
    as a difference from the generated expectation, at the position where the
    two strings first disagree.
    """
    source = "".join(segment.text for segment in segments)
    expected = "".join(segment.masked for segment in segments)
    masked = code_mask(source)
    assert masked == expected, (
        f"mask diverged from the generated expectation at offset "
        f"{_first_difference(masked, expected)} of {len(source)} bytes"
    )


def _first_difference(actual: str, expected: str) -> int:
    """Return the offset of the first differing character, or the length.

    Examples
    --------
    >>> _first_difference("abc", "abc")
    3
    >>> _first_difference("abd", "abc")
    2
    """
    for index, (left, right) in enumerate(zip(actual, expected, strict=False)):
        if left != right:
            return index
    return min(len(actual), len(expected))


@settings(max_examples=200, deadline=None)
@given(source=st.text())
def test_every_mask_keeps_the_offsets_and_the_newlines(source: str) -> None:
    """No input loses a byte, and no input loses a line boundary.

    The module walker maps masked offsets back onto the original text, and the
    inline-module reader counts lines to report where a declaration is. Both
    break silently if masking ever inserts, drops, or rewrites a newline, so
    the claim is asserted for arbitrary text rather than only for sources the
    generator believes are well formed.
    """
    masked = code_mask(source)
    assert len(masked) == len(source), (
        f"masking changed the length from {len(source)} to {len(masked)}"
    )
    assert [
        index for index, character in enumerate(masked) if character == "\n"
    ] == [
        index for index, character in enumerate(source) if character == "\n"
    ], "masking moved a line boundary, so reported positions would be wrong"
    for index, character in enumerate(masked):
        if character != " ":
            assert character == source[index], (
                f"masking rewrote offset {index} instead of blanking it"
            )


@settings(max_examples=100, deadline=None)
@given(source=st.text())
def test_a_mask_is_its_own_fixed_point(source: str) -> None:
    """Masking an already masked source changes nothing.

    The output has no comment or literal opener left, so a second pass has
    nothing to recognise. Anything else would mean the first pass left a
    fragment that a later reader would treat as a construct.
    """
    once = code_mask(source)
    assert code_mask(once) == once, "a second pass over a mask changed it"