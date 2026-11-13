"""What the shared scan primitives do with a path they cannot read.

Every classifier reads through :mod:`source_scan` so that a source which
cannot be read is refused rather than reported as absent. The repository
itself cannot show that: every file in it is readable, so a ``stat_mode`` that
answered ``None`` for any ``OSError`` would agree with the refusing one
everywhere the gate looks, and the difference would only appear on a machine
where a source could not be reached -- which is exactly the case the primitive
exists for.

So the access is denied here, and each primitive is asked the same question:
does it refuse, naming the path, or answer as though the path were not there?

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib

import pytest
from source_scan import ScanError, exists, is_directory, stat_mode
from unreadable_support import DENIED_ACCESS_IS_UNOBSERVABLE, denied

#: The reason attaches to every test that needs a mode to deny something, so
#: the skip reads as the environment's limitation rather than a missing test.
DENIAL_UNOBSERVABLE = "a mode denies a read on neither Windows nor as root"


def test_a_readable_path_is_answered_not_refused() -> None:
    """The positive control: presence and directory-ness are still answered.

    Without this, a ``stat_mode`` that raised for every path would satisfy the
    refusal tests below while making the whole contract unusable.
    """
    here = pathlib.Path(__file__)
    assert stat_mode(here) is not None, (
        "a readable file's mode was refused, so every classifier would fail"
    )
    assert exists(here)() is True, "a readable file was reported as absent"
    assert is_directory(here.parent)() is True, (
        f"the readable directory {here.parent} was not reported as a directory"
    )


def test_an_absent_path_is_an_answer(tmp_path: pathlib.Path) -> None:
    """Only absence is answered; it is not a failure to report.

    A declaration naming no file must contribute nothing rather than take the
    walk down, which is why ``FileNotFoundError`` alone yields ``None``.
    """
    missing = tmp_path / "missing"
    assert stat_mode(missing) is None, (
        "an absent path was not answered as absent, so a missing declaration "
        "would be reported as a scan failure"
    )
    assert exists(missing)() is False, "an absent path was reported as present"
    assert is_directory(missing)() is False, (
        "an absent path was reported as a directory"
    )


@pytest.mark.skipif(DENIED_ACCESS_IS_UNOBSERVABLE, reason=DENIAL_UNOBSERVABLE)
def test_stat_mode_refuses_a_path_it_cannot_reach(tmp_path: pathlib.Path) -> None:
    """A path the process may not search is unreadable, not absent.

    Answering ``None`` here is the false negative the module was written to
    remove: the source behind the unsearchable directory leaves the class in
    silence, and the coverage assertion that should have asked for it never
    does.
    """
    hidden = tmp_path / "hidden"
    hidden.mkdir()
    source = hidden / "source.rs"
    source.write_text("fn f() {}\n", encoding="utf-8")
    with denied(hidden), pytest.raises(ScanError) as raised:
        stat_mode(source)
    assert str(source) in str(raised.value), (
        f"the refusal did not name the path it could not read: {raised.value}"
    )
    assert isinstance(raised.value.__cause__, OSError), (
        f"the original failure was not kept as the cause: {raised.value!r}"
    )


@pytest.mark.skipif(DENIED_ACCESS_IS_UNOBSERVABLE, reason=DENIAL_UNOBSERVABLE)
def test_exists_refuses_rather_than_answering_absent(tmp_path: pathlib.Path) -> None:
    """``exists`` raising is what puts the refusal behind ``scan``'s guard.

    A ``present`` that answered ``False`` for the same ``OSError`` the ``Path``
    predicates swallow would report an unreadable source as a missing one, and
    the guard would never see the failure at all.
    """
    hidden = tmp_path / "hidden"
    hidden.mkdir()
    source = hidden / "source.rs"
    source.write_text("fn f() {}\n", encoding="utf-8")
    with denied(hidden), pytest.raises(ScanError) as raised:
        exists(source)()
    assert str(source) in str(raised.value), (
        f"the refusal did not name the path it could not read: {raised.value}"
    )


@pytest.mark.skipif(DENIED_ACCESS_IS_UNOBSERVABLE, reason=DENIAL_UNOBSERVABLE)
def test_is_directory_refuses_rather_than_answering_not_a_directory(
    tmp_path: pathlib.Path,
) -> None:
    """An unreadable directory is a fault, not a file.

    Reading it as "not a directory" drops the test targets beneath it from the
    inventory, which is the same under-read ``exists`` guards against for
    files.
    """
    hidden = tmp_path / "hidden"
    hidden.mkdir()
    (hidden / "child").mkdir()
    with denied(hidden), pytest.raises(ScanError) as raised:
        is_directory(hidden / "child")()
    assert str(hidden / "child") in str(raised.value), (
        f"the refusal did not name the path it could not read: {raised.value}"
    )