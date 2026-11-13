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
    child_cargo_binaries,
)
from rust_module_graph import ScanError, sources_reached
from unreadable_support import DENIED_ACCESS_IS_UNOBSERVABLE, denied

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
        """Create one empty fixture workspace and return its test directory."""
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


def test_a_parent_climbing_path_attribute_yields_the_plain_source_key(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """A ``#[path]`` containing ``..`` names the same key as a direct reach.

    This repository's ``#[path]`` values climb out of their declaring
    directory, and Rust writes them with ``/`` on every platform. Normalising
    the joined path with POSIX rules leaves the ``..`` in place on Windows,
    where the joined string uses ``\\`` and POSIX rules do not treat it as a
    separator, so one file reached by two routes would appear under two keys.
    The helper's marker is attributed once per key, which is what makes the
    duplicate a misreading rather than a cosmetic difference.

    Wrong answer: a reached set holding ``inner/../shared/helper.rs`` beside
    the plain ``shared/helper.rs``, or only the former.
    """
    tests = crate_tests()
    (tests / "inner").mkdir()
    (tests / "shared").mkdir()
    (tests / "shared" / "helper.rs").write_text(CARRIES, encoding="utf-8")
    (tests / "inner" / "climbing.rs").write_text(
        '#[path = "../shared/helper.rs"]\nmod helper;\n', encoding="utf-8"
    )
    root = tests / "root.rs"
    root.write_text('#[path = "inner/climbing.rs"]\nmod climbing;\n', encoding="utf-8")
    reached = set(sources_reached(root))
    assert reached == {root, tests / "inner" / "climbing.rs", tests / "shared" / "helper.rs"}, (
        "the parent-climbing #[path] did not yield the plain source key: "
        f"{sorted(reached)}"
    )
    assert not [source for source in reached if ".." in source.parts], (
        f"a resolved source key still carries a parent component: {sorted(reached)}"
    )


def test_a_nested_inline_module_is_searched(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """A nested inline module's child is resolved under its module directory.

    ``mod inner { mod leaf; }`` puts ``leaf`` at ``tests/inner/leaf.rs``.
    A root-level ``tests/leaf.rs`` decoy makes a walk that drops the inline
    module from the path fail this test rather than accidentally finding a
    marker in the wrong place.
    """
    tests = crate_tests()
    (tests / "inner").mkdir()
    (tests / "inner" / "leaf.rs").write_text(CARRIES, encoding="utf-8")
    (tests / "leaf.rs").write_text(PLAIN, encoding="utf-8")
    (tests / "inline_nested.rs").write_text(
        'mod inner {\n    const BRACE: &str = "}";\n    /* } */\n'
        "    mod leaf;\n}\n",
        encoding="utf-8",
    )
    root = tests / "inline_nested.rs"
    reached = sources_reached(root)
    assert set(reached) == {root, tests / "inner" / "leaf.rs"}, (
        "the nested module must reach inner/leaf.rs and must not use the "
        "root-level decoy"
    )
    assert carries_child_cargo(root) is True, (
        "removing inline-module traversal must leave the marker unreachable"
    )


def test_a_path_inside_an_inline_module_uses_the_inline_directory(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """``#[path]`` inside ``mod outer`` is relative to ``tests/outer``.

    The Rust Reference resolves an inline module's ``#[path]`` against the
    module directory with the inline names appended, so this declaration loads
    ``tests/outer/x.rs``. Resolving it against the declaring file's directory
    instead finds the root-level decoy, or nothing, and drops a marker Rust
    compiles from the class.
    """
    tests = crate_tests()
    (tests / "outer").mkdir()
    (tests / "outer" / "x.rs").write_text(CARRIES, encoding="utf-8")
    (tests / "x.rs").write_text(PLAIN, encoding="utf-8")
    (tests / "inline_path.rs").write_text(
        'mod outer {\n    #[path = "x.rs"]\n    mod y;\n}\n',
        encoding="utf-8",
    )
    root = tests / "inline_path.rs"
    reached = sources_reached(root)
    assert set(reached) == {root, tests / "outer" / "x.rs"}, (
        "the inline module's #[path] must reach outer/x.rs and must not use "
        "the root-level decoy"
    )
    assert carries_child_cargo(root) is True, (
        "resolving the inline #[path] against the declaring file drops a "
        "marker that Rust compiles"
    )


def test_a_directory_based_test_target_is_in_the_inventory(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """``tests/<name>/main.rs`` is a test target Cargo discovers.

    The file is not a top-level ``tests/*.rs`` file, so an inventory listing
    only those omits the binary. A child-cargo marker in it would then leave
    the class unseen, and a new such binary could stay on the base allowance.
    """
    tests = crate_tests()
    (tests / "grouped").mkdir()
    (tests / "grouped" / "main.rs").write_text(CARRIES, encoding="utf-8")
    binaries = child_cargo_binaries(tests.parents[1])
    assert "grouped" in binaries, (
        "tests/grouped/main.rs was not inventoried, so its marker left the "
        f"class: {sorted(binaries)}"
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


def test_an_ambiguous_declaration_is_refused(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """``mod child;`` may not resolve to both ``child.rs`` and ``child/mod.rs``.

    Rust forbids both files existing, so a tree holding them is malformed. A
    reader that picked one -- the first, or the one its search order happened
    to reach -- would compile a binary from a source Rust refuses to compile,
    and the marker in the other file would be attributed or dropped by a coin
    toss. Refusing names the declaring file and both candidates, which is what
    a maintainer needs to see which declaration is at fault.
    """
    tests = crate_tests()
    (tests / "child.rs").write_text(PLAIN, encoding="utf-8")
    (tests / "child").mkdir()
    (tests / "child" / "mod.rs").write_text(PLAIN, encoding="utf-8")
    root = tests / "ambiguous.rs"
    root.write_text("mod child;\n", encoding="utf-8")
    with pytest.raises(ScanError) as raised:
        sources_reached(root)
    message = str(raised.value)
    assert str(root) in message, (
        f"the refusal did not name the declaring file: {message}"
    )
    assert "child.rs" in message and "mod.rs" in message, (
        f"the refusal did not name both candidates, so it cannot be acted on: {message}"
    )


@pytest.mark.skipif(
    DENIED_ACCESS_IS_UNOBSERVABLE,
    reason="a mode denies a read on neither Windows nor as root",
)
def test_an_unreadable_declared_source_is_refused(
    crate_tests: typ.Callable[[], pathlib.Path],
) -> None:
    """A declared source that cannot be opened is a fault, not an absence.

    The declaration resolves to a file that is there, so a reader that treated
    the failed read as "nothing to contribute" would report the binary as
    carrying no marker and leave the class a member short -- the under-read
    the contract exists to catch, and one no readable fixture can show.
    """
    tests = crate_tests()
    declared = tests / "child.rs"
    declared.write_text(CARRIES, encoding="utf-8")
    root = tests / "unreadable.rs"
    root.write_text("mod child;\n", encoding="utf-8")
    with denied(declared), pytest.raises(ScanError) as raised:
        sources_reached(root)
    assert str(declared) in str(raised.value), (
        f"the refusal did not name the source it could not read: {raised.value}"
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
    assert carries_child_cargo(tests / "first_user.rs") is True, (
        "the helper's marker reached the second binary but not the first, so "
        "the attributed set is asymmetric rather than shared"
    )
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
