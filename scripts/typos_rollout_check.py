#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = []
# ///
"""Enforce exact phrase corrections alongside the Typos scanner."""

from __future__ import annotations
import argparse
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
import re
import subprocess
import generate_typos_config as generator
import typos_rollout as rollout
import typos_rollout_policy as policy_validation

POLICY_PATHS = frozenset({Path(".typos-oxendict-base.toml"), Path("typos.local.toml")})


@dataclass(frozen=True)
class PhraseFinding:
    """Describe one prohibited phrase in tracked text.

    Attributes
    ----------
    path
        Repository-relative file containing the phrase.
    line
        One-based source line.
    column
        One-based source column.
    phrase
        Original source spelling.
    correction
        Required replacement spelling.
    """

    path: Path
    line: int
    column: int
    phrase: str
    correction: str


def _tracked(repository: Path) -> tuple[Path, ...]:
    """Return tracked paths in deterministic order."""
    raw = subprocess.run(
        ["git", "-C", str(repository), "ls-files", "-z"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return tuple(Path(item) for item in sorted(filter(None, raw.split("\0"))))


def _excluded(path: Path, dictionary: rollout.Dictionary) -> bool:
    """Return whether policy excludes a repository-relative path."""
    return any(
        item in path.parts or path.match(item) for item in dictionary.excluded_files
    )


def _masked(text: str, patterns: tuple[str, ...]) -> str:
    """Blank safely bounded policy matches while preserving positions."""

    def blank(match: re.Match[str]) -> str:
        """Replace non-newline match characters with spaces."""
        return "".join("\n" if c == "\n" else " " for c in match.group())

    for pattern in policy_validation.compile_ignore_patterns(patterns):
        text = pattern.sub(blank, text)
    return text


def _phrase_findings(
    path: Path,
    text: str,
    masked: str,
    policy: tuple[str, str],
) -> tuple[PhraseFinding, ...]:
    """Find one prohibited phrase in position-preserving masked text."""
    phrase, correction = policy
    found = []
    for match in re.finditer(
        rf"(?<![\w-]){re.escape(phrase)}(?![\w-])",
        masked,
        re.IGNORECASE,
    ):
        previous = masked.rfind("\n", 0, match.start())
        found.append(
            PhraseFinding(
                path,
                masked.count("\n", 0, match.start()) + 1,
                match.start() - previous,
                text[match.start() : match.end()],
                correction,
            )
        )
    return tuple(found)


def _file_findings(
    repository: Path,
    relative: Path,
    dictionary: rollout.Dictionary,
) -> tuple[PhraseFinding, ...]:
    """Find all prohibited phrases in one eligible tracked UTF-8 file."""
    if relative in POLICY_PATHS or _excluded(relative, dictionary):
        return ()
    try:
        text = (repository / relative).read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return ()
    masked = _masked(text, dictionary.ignore_patterns)
    return tuple(
        finding
        for policy in dictionary.phrase_corrections
        for finding in _phrase_findings(relative, text, masked, policy)
    )


def check_phrase_corrections(
    repository: Path, dictionary: rollout.Dictionary
) -> tuple[PhraseFinding, ...]:
    """Find prohibited exact phrases in tracked UTF-8 text.

    Parameters
    ----------
    repository
        Git repository whose tracked files are scanned.
    dictionary
        Validated spelling policy containing phrases and exclusions.

    Returns
    -------
    tuple[PhraseFinding, ...]
        Findings ordered by tracked path and phrase policy order.

    Raises
    ------
    OSError, subprocess.CalledProcessError, ValueError
        If repository discovery, file reading or regex validation fails.
    """
    return tuple(
        finding
        for relative in _tracked(repository)
        for finding in _file_findings(repository, relative, dictionary)
    )


def main(argv: Sequence[str] | None = None) -> int:
    """Run the phrase checker command.

    Parameters
    ----------
    argv
        Optional command-line arguments, excluding the executable name.

    Returns
    -------
    int
        ``2`` when findings exist, otherwise ``0``.

    Raises
    ------
    OSError, subprocess.CalledProcessError, TypeError, ValueError
        If policy loading or repository scanning fails.
    """
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", type=Path, default=Path.cwd())
    repository = parser.parse_args(argv).repository
    findings = check_phrase_corrections(
        repository, generator.dictionary_from_cache(repository)
    )
    for item in findings:
        print(
            f"{item.path}:{item.line}:{item.column}: {item.phrase} -> {item.correction}"
        )
    return 2 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
