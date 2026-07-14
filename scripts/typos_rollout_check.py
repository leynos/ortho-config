#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = []
# ///
"""Enforce exact phrase corrections alongside the Typos scanner."""

from __future__ import annotations
import argparse
from dataclasses import dataclass
from pathlib import Path
import re
import subprocess
from typing import Sequence
import generate_typos_config as generator
import typos_rollout as rollout

POLICY_PATHS = frozenset({Path(".typos-oxendict-base.toml"), Path("typos.local.toml")})


@dataclass(frozen=True)
class PhraseFinding:
    """Describe one prohibited phrase in tracked text."""

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
    return any(
        item in path.parts or path.match(item) for item in dictionary.excluded_files
    )


def _masked(text: str, patterns: tuple[str, ...]) -> str:
    def blank(match: re.Match[str]) -> str:
        return "".join("\n" if c == "\n" else " " for c in match.group())

    for pattern in patterns:
        text = re.sub(pattern, blank, text)
    return text


def check_phrase_corrections(
    repository: Path, dictionary: rollout.Dictionary
) -> tuple[PhraseFinding, ...]:
    """Find prohibited exact phrases in tracked UTF-8 text."""
    found = []
    for relative in _tracked(repository):
        if relative in POLICY_PATHS or _excluded(relative, dictionary):
            continue
        try:
            text = (repository / relative).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        masked = _masked(text, dictionary.ignore_patterns)
        for phrase, correction in dictionary.phrase_corrections:
            for match in re.finditer(
                rf"(?<![\w-]){re.escape(phrase)}(?![\w-])", masked, re.IGNORECASE
            ):
                previous = masked.rfind("\n", 0, match.start())
                found.append(
                    PhraseFinding(
                        relative,
                        masked.count("\n", 0, match.start()) + 1,
                        match.start() - previous,
                        text[match.start() : match.end()],
                        correction,
                    )
                )
    return tuple(found)


def main(argv: Sequence[str] | None = None) -> int:
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
