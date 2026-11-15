"""Recognizing this test tier's child-cargo source markers.

This module owns the marker labels and matches them against files a Rust test
binary compiles, using :mod:`rust_module_graph` for reachability and raw source
text for the marker patterns. It is specific to the child-cargo tier;
contracts needing the class should import :mod:`child_cargo_tier` rather than
maintaining another set of marker patterns.

The text match describes this repository's source idioms, not Rust execution.
It can match comments or unused code, while module traversal keeps unrelated
helpers and command builders out of the binary's membership.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

from rust_module_graph import ScanError, sources_reached

__all__ = ["CARGO_MARKERS", "ScanError", "carries_child_cargo", "marker_sources"]

#: What it looks like when a source starts a child `cargo`, by label. Three
#: shapes, because the repository has three: a direct spawn, a spawn routed
#: through the shared cargo-runner helper, and the orthohelp binary loader
#: every test drives the built tool through.
#:
#: :func:`child_cargo_tier.marker_binaries` reports each label's own set, and
#: the contract asserts every one of them is non-empty. A marker that has gone
#: dead -- because the helper it names was renamed -- would silently shrink the
#: class and let the coverage assertion pass over a binary that is no longer
#: read at all, so the sets are asserted rather than assumed. A bare
#: `prepare_cargo_command` marker did exactly that after the helper it named was
#: renamed, and the replacement carries the old name in its pattern so the
#: rename cannot recur silently.
#: The direct-spawn pattern excludes a ``clap::``-qualified ``Command``, which
#: is a *builder*: ``clap::Command::new("cargo")`` names a command for a help
#: screen and starts no process. Reading it as a spawn puts a binary in the
#: class that pays no nested build, and the contract then reports the
#: configuration's filter as too wide -- blaming the configuration for a
#: misreading of the Rust. Only the ``clap`` path is excluded: a
#: ``std::process::`` qualification is a real spawn and is still read. The
#: repository has one such builder, in ``ortho_config/src/cargo/mod.rs``,
#: outside every test binary, so the exclusion changes no member today; it is
#: here so that a test file using clap cannot be misread as spawning cargo.
CARGO_MARKERS: typ.Final[tuple[tuple[str, re.Pattern[str]], ...]] = (
    ("cargo-spawn", re.compile(r'(?<!clap::)Command::new\(\s*"cargo"\s*\)')),
    ("binary-loader", re.compile(r"\bcargo_orthohelp_exe\b")),
    (
        "cargo-runner",
        re.compile(r"\bprepare_cargo_command_for_target_before\b|\bcargo_command\b"),
    ),
)


def marker_sources(root: Path) -> dict[str, list[Path]]:
    """Return marker labels found in files compiled by ``root``.

    Examples
    --------
    A root that is not there compiles nothing:

    >>> marker_sources(Path("/nonexistent.rs"))
    {}
    """
    found: dict[str, list[Path]] = {}
    for source, text in sources_reached(root).items():
        for label, marker in CARGO_MARKERS:
            if marker.search(text):
                found.setdefault(label, []).append(source)
    return found


def carries_child_cargo(root: Path) -> bool:
    """Report whether compiling the binary rooted at ``root`` spawns cargo.

    Parameters
    ----------
    root : pathlib.Path
        The crate root of one test binary.

    Returns
    -------
    bool
        Whether any file ``root`` compiles carries a marker. A ``mod`` it
        declares that lives outside the crate still counts, because ``#[path]``
        can name one.

    Raises
    ------
    ScanError
        If a source the binary compiles cannot be read, or a ``mod``
        declaration resolves to more than one file.

    Examples
    --------
    A source with no marker does not carry one:

    >>> carries_child_cargo(Path("/nonexistent.rs"))
    False
    """
    return bool(marker_sources(root))
