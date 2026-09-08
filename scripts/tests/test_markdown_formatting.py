"""Exercise the tracked-Markdown formatter scripts at their process boundary."""

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WITH_TRACKED_MARKDOWN = REPOSITORY_ROOT / "scripts" / "with-tracked-markdown.sh"
FORMAT_MARKDOWN = REPOSITORY_ROOT / "scripts" / "format-tracked-markdown.sh"
CHECK_MARKDOWN = REPOSITORY_ROOT / "scripts" / "check-markdown-format.sh"
MDTABLEFIX_FLAGS = [
    "--wrap",
    "--renumber",
    "--breaks",
    "--ellipsis",
    "--fences",
    "--in-place",
]


def run_command(
    arguments: list[str],
    directory: Path,
    environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run one controlled formatter command."""
    return subprocess.run(
        arguments,
        capture_output=True,
        check=False,
        cwd=directory,
        env=os.environ | (environment or {}),
        text=True,
    )


def write_executable(path: Path, source: str) -> Path:
    """Write an executable test double and return its path."""
    path.write_text(source.replace("__PYTHON__", sys.executable), encoding="utf-8")
    path.chmod(0o755)
    return path


def initialise_repository(path: Path) -> None:
    """Create a minimal index so tracked-file selection can be observed."""
    run_command(["git", "init", "--quiet"], path)


def test_selects_every_tracked_regular_markdown_file(tmp_path: Path) -> None:
    """Include hidden and fixture-named files while excluding untracked files."""
    initialise_repository(tmp_path)
    hidden = tmp_path / ".hidden.md"
    fixture = tmp_path / "tests" / "fixtures" / "literal [bracket]*?.md"
    regular = tmp_path / "regular.md"
    untracked = tmp_path / "untracked.md"
    fixture.parent.mkdir(parents=True)
    for path in (hidden, fixture, regular, untracked):
        path.write_text("# heading\n", encoding="utf-8")
    run_command(
        [
            "git",
            "add",
            ".hidden.md",
            "tests/fixtures/literal [bracket]*?.md",
            "regular.md",
        ],
        tmp_path,
    )

    arguments_file = tmp_path / "arguments.bin"
    command = write_executable(
        tmp_path / "record-arguments",
        '#!/usr/bin/env bash\nprintf \'%s\\0\' "$@" > "$ARGS_OUT"\n',
    )
    result = run_command(
        [str(WITH_TRACKED_MARKDOWN), str(command)],
        tmp_path,
        {"ARGS_OUT": str(arguments_file)},
    )

    assert result.returncode == 0, result.stderr
    selected = arguments_file.read_bytes().split(b"\0")[:-1]
    assert selected[0] == b"--"
    assert set(selected[1:]) == {
        b".hidden.md",
        b"regular.md",
        b"tests/fixtures/literal [bracket]*?.md",
    }


def test_excludes_a_tracked_symlink_without_following_it(tmp_path: Path) -> None:
    """Avoid duplicate or out-of-tree writes through a tracked Markdown link."""
    initialise_repository(tmp_path)
    document = tmp_path / "document.md"
    document.write_text("# heading\n", encoding="utf-8")
    link = tmp_path / "linked.md"
    try:
        link.symlink_to(document.name)
    except OSError as error:
        pytest.skip(f"symlink creation is unavailable: {error}")
    run_command(["git", "add", "document.md", "linked.md"], tmp_path)

    arguments_file = tmp_path / "arguments.bin"
    command = write_executable(
        tmp_path / "record-arguments",
        '#!/usr/bin/env bash\nprintf \'%s\\0\' "$@" > "$ARGS_OUT"\n',
    )
    result = run_command(
        [str(WITH_TRACKED_MARKDOWN), str(command)],
        tmp_path,
        {"ARGS_OUT": str(arguments_file)},
    )

    assert result.returncode == 0, result.stderr
    assert arguments_file.read_bytes().split(b"\0")[:-1] == [b"--", b"document.md"]


def test_propagates_the_selected_command_failure(tmp_path: Path) -> None:
    """Keep a formatter failure binding on the enclosing Make recipe."""
    initialise_repository(tmp_path)
    source = tmp_path / "source.md"
    source.write_text("# heading\n", encoding="utf-8")
    run_command(["git", "add", "source.md"], tmp_path)
    command = write_executable(tmp_path / "fail", "#!/usr/bin/env bash\nexit 42\n")

    result = run_command([str(WITH_TRACKED_MARKDOWN), str(command)], tmp_path)

    assert result.returncode == 42


def test_returns_nonzero_when_git_root_discovery_fails(tmp_path: Path) -> None:
    """Do not treat an unmeasurable source set as an empty successful one."""
    command = write_executable(tmp_path / "success", "#!/usr/bin/env bash\nexit 0\n")

    result = run_command([str(WITH_TRACKED_MARKDOWN), str(command)], tmp_path)

    assert result.returncode != 0


def write_mdtablefix(directory: Path, calls: Path) -> Path:
    """Create a controlled mdtablefix replacement that records canonical flags."""
    return write_executable(
        directory / "mdtablefix",
        """#!/usr/bin/env python3
import json
import os
import pathlib
import sys

arguments = sys.argv[1:]
with pathlib.Path(os.environ["MDTABLEFIX_CALLS"]).open("a") as output:
    print(json.dumps(arguments), file=output)
for source in arguments[6:]:
    path = pathlib.Path(source)
    path.write_bytes(path.read_bytes().replace(b"unformatted", b"formatted"))
""",
    )


def write_markdownlint(directory: Path, calls: Path) -> Path:
    """Create a controlled markdownlint-cli2 replacement for process tests."""
    return write_executable(
        directory / "markdownlint-cli2",
        """#!/usr/bin/env python3
import json
import os
import pathlib
import sys

arguments = sys.argv[1:]
with pathlib.Path(os.environ["MARKDOWNLINT_CALLS"]).open("a") as output:
    print(json.dumps(arguments), file=output)
if "--fix" not in arguments:
    raise SystemExit(1)
raise SystemExit(0)
""",
    )


def test_markdownlint_options_keep_root_formatting_universal() -> None:
    """Keep ordinary lint exclusions out of the automatically loaded options."""
    root_options = json.loads(
        (REPOSITORY_ROOT / ".markdownlint-cli2.jsonc").read_text()
    )
    normal_options = json.loads(
        (REPOSITORY_ROOT / ".markdownlint-cli2-normal.jsonc").read_text()
    )

    rules = json.loads((REPOSITORY_ROOT / ".markdownlint.jsonc").read_text())
    assert rules == {
        "MD004": {"style": "dash"},
        "MD010": {"code_blocks": False},
        "MD013": {
            "line_length": 80,
            "code_block_line_length": 120,
            "tables": False,
            "headings": False,
        },
        "MD029": {"style": "ordered"},
    }
    assert root_options == {"config": {"extends": ".markdownlint.jsonc"}}
    assert normal_options["config"]["extends"] == ".markdownlint.jsonc"
    assert normal_options["ignores"] == [
        "**/.venv/**",
        "**/node_modules/**",
        "**/target/**",
        "**/.git/**",
    ]


def test_formatter_uses_the_canonical_tool_arguments(tmp_path: Path) -> None:
    """Keep the source formatter flags and Markdownlint fix pass in order."""
    source = tmp_path / "source.md"
    source.write_text("# heading\n", encoding="utf-8")
    mdtablefix_calls = tmp_path / "mdtablefix.jsonl"
    markdownlint_calls = tmp_path / "markdownlint.jsonl"
    mdtablefix = write_mdtablefix(tmp_path, mdtablefix_calls)
    markdownlint = write_markdownlint(tmp_path, markdownlint_calls)

    result = run_command(
        [str(FORMAT_MARKDOWN), str(mdtablefix), str(markdownlint), "--", str(source)],
        tmp_path,
        {
            "MDTABLEFIX_CALLS": str(mdtablefix_calls),
            "MARKDOWNLINT_CALLS": str(markdownlint_calls),
        },
    )

    assert result.returncode == 0, result.stderr
    mdtablefix_arguments = [
        json.loads(line) for line in mdtablefix_calls.read_text().splitlines()
    ]
    markdownlint_arguments = [
        json.loads(line) for line in markdownlint_calls.read_text().splitlines()
    ]
    assert mdtablefix_arguments == [MDTABLEFIX_FLAGS + [str(source)]]
    assert markdownlint_arguments == [
        [
            "--fix",
            "--no-globs",
            "--config",
            ".markdownlint-cli2.jsonc",
            "--",
            str(source),
        ],
    ]


def test_formatter_propagates_the_first_formatter_failure(tmp_path: Path) -> None:
    """Stop before Markdownlint when mdtablefix cannot format a selected file."""
    source = tmp_path / "source.md"
    source.write_text("# heading\n", encoding="utf-8")
    mdtablefix = write_executable(
        tmp_path / "mdtablefix", "#!/usr/bin/env bash\nexit 31\n"
    )
    markdownlint_calls = tmp_path / "markdownlint.jsonl"
    markdownlint = write_markdownlint(tmp_path, markdownlint_calls)

    result = run_command(
        [str(FORMAT_MARKDOWN), str(mdtablefix), str(markdownlint), "--", str(source)],
        tmp_path,
        {"MARKDOWNLINT_CALLS": str(markdownlint_calls)},
    )

    assert result.returncode == 31
    assert not markdownlint_calls.exists()


def test_formatter_propagates_the_markdownlint_failure(tmp_path: Path) -> None:
    """Keep the Markdownlint fix failure binding after mdtablefix succeeds."""
    source = tmp_path / "source.md"
    source.write_text("# heading\n", encoding="utf-8")
    mdtablefix_calls = tmp_path / "mdtablefix.jsonl"
    mdtablefix = write_mdtablefix(tmp_path, mdtablefix_calls)
    markdownlint = write_executable(
        tmp_path / "markdownlint-cli2", "#!/usr/bin/env bash\nexit 32\n"
    )

    result = run_command(
        [str(FORMAT_MARKDOWN), str(mdtablefix), str(markdownlint), "--", str(source)],
        tmp_path,
        {"MDTABLEFIX_CALLS": str(mdtablefix_calls)},
    )

    assert result.returncode == 32
    assert mdtablefix_calls.exists()


def test_checker_accepts_crlf_without_modifying_sources(tmp_path: Path) -> None:
    """Check staged copies and accept either canonical Git line-ending form."""
    mdtablefix_calls = tmp_path / "mdtablefix.jsonl"
    mdtablefix = write_mdtablefix(tmp_path, mdtablefix_calls)
    lf_source = tmp_path / "lf.md"
    crlf_source = tmp_path / "crlf.md"
    lf_source.write_bytes(b"formatted\n")
    crlf_source.write_bytes(b"formatted\r\n")

    result = run_command(
        [str(CHECK_MARKDOWN), str(lf_source), str(crlf_source)],
        tmp_path,
        {"MDTABLEFIX": str(mdtablefix), "MDTABLEFIX_CALLS": str(mdtablefix_calls)},
    )

    assert result.returncode == 0, result.stderr
    assert lf_source.read_bytes() == b"formatted\n"
    assert crlf_source.read_bytes() == b"formatted\r\n"
    calls = [json.loads(line) for line in mdtablefix_calls.read_text().splitlines()]
    assert calls[0][:6] == ["--in-place", *MDTABLEFIX_FLAGS[:-1]]


def test_selector_composes_with_checker_and_literal_file_names(tmp_path: Path) -> None:
    """Pass the Make formatter argument through the selector separator safely."""
    initialise_repository(tmp_path)
    source = tmp_path / "with space - dash.md"
    source.write_bytes(b"formatted\n")
    run_command(["git", "add", source.name], tmp_path)
    calls = tmp_path / "mdtablefix.jsonl"
    mdtablefix = write_mdtablefix(tmp_path, calls)

    result = run_command(
        [
            str(WITH_TRACKED_MARKDOWN),
            str(CHECK_MARKDOWN),
            "--formatter",
            str(mdtablefix),
        ],
        tmp_path,
        {"MDTABLEFIX_CALLS": str(calls)},
    )

    assert result.returncode == 0, result.stderr
    assert source.read_bytes() == b"formatted\n"


def test_checker_reports_noncanonical_sources_without_mutating_them(
    tmp_path: Path,
) -> None:
    """Reject a reformattable source while preserving its original bytes."""
    mdtablefix_calls = tmp_path / "mdtablefix.jsonl"
    mdtablefix = write_mdtablefix(tmp_path, mdtablefix_calls)
    source = tmp_path / "source.md"
    source.write_bytes(b"unformatted\n")

    result = run_command(
        [str(CHECK_MARKDOWN), str(source)],
        tmp_path,
        {"MDTABLEFIX": str(mdtablefix), "MDTABLEFIX_CALLS": str(mdtablefix_calls)},
    )

    assert result.returncode == 1
    assert str(source) in result.stderr
    assert source.read_bytes() == b"unformatted\n"


def test_checker_propagates_the_mdtablefix_failure(tmp_path: Path) -> None:
    """Fail the enclosing formatting check when its copy formatter fails."""
    source = tmp_path / "source.md"
    source.write_bytes(b"formatted\n")
    mdtablefix = write_executable(
        tmp_path / "mdtablefix", "#!/usr/bin/env bash\nexit 41\n"
    )

    result = run_command(
        [str(CHECK_MARKDOWN), str(source)], tmp_path, {"MDTABLEFIX": str(mdtablefix)}
    )

    assert result.returncode == 41
    assert source.read_bytes() == b"formatted\n"


def test_checker_rejects_symlinks_without_following_them(tmp_path: Path) -> None:
    """Keep direct checker use within the same no-symlink contract as selection."""
    document = tmp_path / "document.md"
    document.write_bytes(b"formatted\n")
    link = tmp_path / "linked.md"
    try:
        link.symlink_to(document.name)
    except OSError as error:
        pytest.skip(f"symlink creation is unavailable: {error}")
    mdtablefix = write_executable(
        tmp_path / "mdtablefix", "#!/usr/bin/env bash\nexit 0\n"
    )

    result = run_command(
        [str(CHECK_MARKDOWN), str(link)], tmp_path, {"MDTABLEFIX": str(mdtablefix)}
    )

    assert result.returncode == 1
    assert document.read_bytes() == b"formatted\n"
