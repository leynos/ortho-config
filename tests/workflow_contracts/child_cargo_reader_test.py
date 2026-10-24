"""How the child-cargo contract reads the Rust it takes its class from.

Every assertion in ``child_cargo_tier_test`` rests on two readings: which
binaries exist, and which of them compile a source that starts a child cargo.
This repository's own sources exercise both, but they cannot exercise most of
the shapes the readings can *get wrong*, because a repository that reads
correctly everywhere cannot demonstrate that a misreading would be caught.

So the reader is driven here with controlled inputs, one per shape that has a
plausible right answer and a plausible wrong one. Each test states what the
wrong answer would have been, which is what makes it a test of the reading
rather than a second copy of the assertion it protects.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import typing as typ

import pytest
from child_cargo_tier import (
    CHILD_CARGO_TIER_CRATES,
    carries_child_cargo,
)
from rust_module_graph import sources_reached

#: A source that starts a child ``cargo``, and one that does not. The pair is
#: what makes each assertion below a reading test rather than a smoke test:
#: every fixture is checked for the marker it was written to carry, and the
#: negative control checks that the other shape is not reported.
CARRIES: typ.Final[str] = 'fn run() {\n    let _ = Command::new("cargo");\n}\n'
PLAIN: typ.Final[str] = "fn run() {}\n"


@pytest.fixture
def crate_tests(tmp_path: pathlib.Path) -> typ.Callable[[], pathlib.Path]:
    """Return a factory writing a fresh fixture workspace's ``tests/``.

    A factory rather than a single directory because the fixtures below add
    files with the same names to the same kind of tree, and a shared directory
    would let one test's leftovers satisfy -- or break -- another's reading.
    Only the first audited crate is populated; the rest exist so the reader's
    fixed crate list finds a directory rather than nothing.

    Returns
    -------
    callable
        Called with no arguments, returns the ``tests`` directory of a new
        workspace.
    """

    def write() -> pathlib.Path:
        for crate in CHILD_CARGO_TIER_CRATES:
            (tmp_path / crate / "tests").mkdir(parents=True, exist_ok=True)
            (tmp_path / crate / "Cargo.toml").write_text(
                "[package]\n", encoding="utf-8"
            )
        return tmp_path / CHILD_CARGO_TIER_CRATES[0] / "tests"

    return write


def test_a_declared_path_module_is_followed_from_its_owner(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """``#[path]`` names the file relative to the file declaring it.

    A reader resolving it against the crate root, or against the ``tests``
    directory, looks for the file somewhere it is not, finds nothing, and
    reports the binary as carrying no child cargo -- the reading that shrinks
    the class. The target sits in a subdirectory, so the two resolutions
    genuinely disagree rather than happening to coincide.
    """
    tests = crate_tests()
    (tests / "nested").mkdir()
    (tests / "nested" / "target.rs").write_text(CARRIES, encoding="utf-8")
    (tests / "renamed.rs").write_text(
        '#[path = "nested/target.rs"]\nmod elsewhere;\n', encoding="utf-8"
    )
    assert carries_child_cargo(tests / "renamed.rs") is True, (
        "the #[path] target was not followed from the declaring file, so the "
        "marker in it was not read and the binary left the class"
    )


def test_a_path_module_keeps_its_own_siblings(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """A ``#[path]`` target's own submodules resolve beside the target.

    The Rust Reference makes the loaded file the owner of its own children.
    A reader that resolved the target's children against the ``tests``
    directory instead would look for this sibling in the wrong place and miss
    the marker, which is the same under-read as resolving ``#[path]`` wrongly.
    """
    tests = crate_tests()
    (tests / "via_path").mkdir()
    (tests / "via_path" / "entry.rs").write_text("mod child;\n", encoding="utf-8")
    (tests / "via_path" / "child.rs").write_text(CARRIES, encoding="utf-8")
    (tests / "path_root.rs").write_text(
        '#[path = "via_path/entry.rs"]\nmod entry;\n', encoding="utf-8"
    )
    assert carries_child_cargo(tests / "path_root.rs") is True, (
        "the #[path] target's own submodule was not resolved beside the "
        "target, so a marker one level down was not read"
    )


def test_a_nested_inline_module_is_searched(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """A ``mod`` block's children are in scope for the containing file.

    ``mod inner { mod leaf; }`` puts ``leaf`` beside the *containing file*,
    not inside a directory named after the block. A reader that derived a
    directory from the inline module's name would look for ``inner/leaf.rs``
    and miss the marker.
    """
    tests = crate_tests()
    (tests / "leaf.rs").write_text(CARRIES, encoding="utf-8")
    (tests / "inline_nested.rs").write_text(
        "mod inner {\n    mod leaf;\n}\n", encoding="utf-8"
    )
    assert carries_child_cargo(tests / "inline_nested.rs") is True, (
        "the inline module's child was looked for in a directory named after "
        "the block rather than beside the containing file"
    )


def test_a_missing_declaration_is_an_absence_not_a_failure(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """A declaration naming no file contributes nothing, and does not raise.

    An unresolvable declaration must not take the whole walk down: the
    contract would then fail with a scan error naming a file rather than
    reporting the class, and a genuine omission elsewhere would be hidden
    behind it.
    """
    tests = crate_tests()
    (tests / "unresolved.rs").write_text(
        '#[path = "does/not/exist.rs"]\nmod gone;\n\nfn f() {}\n',
        encoding="utf-8",
    )
    reached = sources_reached(tests / "unresolved.rs")
    assert (tests / "unresolved.rs") in reached, (
        "the crate root itself must always be among the sources reached"
    )
    assert len(reached) == 1, (
        f"a declaration naming no existing file contributed {sorted(reached)}; "
        f"only the root should be reached"
    )


def test_a_shared_helper_is_attributed_to_every_binary_reaching_it(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """A marker in a shared module counts for each binary that compiles it.

    This is what makes the class a property of the module graph rather than of
    each file's own text, and it is the reading the marker sets rest on: a
    binary whose own file is clean but whose helper spawns cargo does drive a
    child cargo.
    """
    tests = crate_tests()
    (tests / "shared").mkdir()
    (tests / "shared" / "mod.rs").write_text(CARRIES, encoding="utf-8")
    for binary in ("first_user", "second_user"):
        (tests / f"{binary}.rs").write_text("mod shared;\n", encoding="utf-8")
    assert carries_child_cargo(tests / "first_user.rs") is True
    assert carries_child_cargo(tests / "second_user.rs") is True, (
        "the helper's marker was attributed to one binary but not the other"
    )


def test_a_clap_command_builder_is_not_a_cargo_spawn(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """``clap::Command::new("cargo")`` builds a help screen; it starts nothing.

    The pattern is matched against raw source text, so without a boundary
    before ``Command`` the clap builder reads as a spawn and puts a binary in
    the class that pays no nested build. The contract's over-wide assertion
    then reports the *configuration* as granting an allowance the class does
    not need, so the misreading surfaces as a false accusation against a
    correct filter rather than as a missing member.

    The repository has exactly one such call,
    ``ortho_config/src/cargo/mod.rs:126``, in a non-test module, so nothing in
    the certified class exercises this shape today.
    """
    tests = crate_tests()
    (tests / "qualified.rs").write_text(
        'fn build() {\n    let _ = clap::Command::new("cargo");\n}\n',
        encoding="utf-8",
    )
    assert carries_child_cargo(tests / "qualified.rs") is False, (
        "a clap builder was read as a child-cargo spawn, so the class includes "
        "a binary that starts no child process"
    )


def test_a_qualified_process_command_is_still_a_spawn(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """Excluding the clap builder must not exclude the real spawn.

    ``std::process::Command::new("cargo")`` is the same call as the bare form
    with a longer path, and it starts cargo. A boundary broad enough to
    exclude every qualified ``Command`` would satisfy the test above while
    dropping genuine members from the class -- the under-read, which is the
    defect the contract exists to catch.
    """
    tests = crate_tests()
    (tests / "qualified_spawn.rs").write_text(
        'fn run() {\n    let _ = std::process::Command::new("cargo");\n}\n',
        encoding="utf-8",
    )
    assert carries_child_cargo(tests / "qualified_spawn.rs") is True, (
        "a std::process-qualified Command::new(\"cargo\") was not read as a "
        "spawn, so a genuine class member was dropped"
    )


def test_a_plain_binary_reports_no_marker(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """The negative control for the marker reading itself.

    Without it, a ``carries_child_cargo`` that returned True for everything
    would satisfy every assertion above and put the whole workspace in the
    class.
    """
    tests = crate_tests()
    (tests / "plain.rs").write_text(PLAIN, encoding="utf-8")
    assert carries_child_cargo(tests / "plain.rs") is False, (
        "a source with no cargo marker was reported as driving a child cargo"
    )
