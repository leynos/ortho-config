"""Verify Markdown formatter pins and isolated tool provisioning."""

import json
import os
import re
import subprocess
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MDTABLEFIX = REPOSITORY_ROOT / "scripts" / "mdtablefix.sh"
PROVISION_MDTABLEFIX = REPOSITORY_ROOT / "scripts" / "provision-mdtablefix.sh"


def run_command(
    arguments: list[str], directory: Path, environment: dict[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    """Run one controlled provisioning command."""
    return subprocess.run(
        arguments,
        capture_output=True,
        check=False,
        cwd=directory,
        env=os.environ | (environment or {}),
        text=True,
    )


def write_executable(path: Path, source: str) -> Path:
    """Write a test double executable and return its path."""
    path.write_text(source, encoding="utf-8")
    path.chmod(0o755)
    return path


def test_ci_and_makefile_use_the_same_mdtablefix_pin() -> None:
    """Prevent CI from validating a different formatter release than local Make."""
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    workflow = (REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml").read_text(
        encoding="utf-8"
    )
    make_version = re.search(
        r"^MDTABLEFIX_VERSION \?= ([0-9.]+)$", makefile, re.MULTILINE
    )
    workflow_version = re.search(
        r"^ {6}MDTABLEFIX_VERSION: '([0-9.]+)'$", workflow, re.MULTILINE
    )

    assert make_version is not None
    assert workflow_version is not None
    assert workflow_version.group(1) == make_version.group(1)
    assert "bin-dir: ${{ env.MDTABLEFIX_BIN_DIR }}" in workflow


def test_make_and_ci_preserve_the_normal_markdown_exclusions() -> None:
    """Keep ordinary lint exclusions explicit while formatting remains universal."""
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    workflow = (REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml").read_text(
        encoding="utf-8"
    )
    normal_options = json.loads(
        (REPOSITORY_ROOT / ".markdownlint-cli2-normal.jsonc").read_text()
    )
    exclusions = normal_options["ignores"]

    assert '$(MDLINT) --config "$(MDLINT_NORMAL_CONFIG)" "**/*.md"' in makefile
    for exclusion in exclusions:
        assert exclusion in workflow


def test_evaluated_make_preserves_linux_and_windows_suffix_routes() -> None:
    """Keep the local empty suffix and wrapper-selected Windows suffix distinct."""
    result = run_command(
        [
            "make",
            "--dry-run",
            "provision-mdtablefix",
            "CARGO=probe-cargo",
            "MDTABLEFIX_VERSION=0.5.1",
            "MDTABLEFIX_BIN_DIR=/tmp/mdtablefix-linux",
        ],
        REPOSITORY_ROOT,
    )
    assert result.returncode == 0, result.stderr
    assert (
        'scripts/provision-mdtablefix.sh "probe-cargo" "0.5.1" '
        '"/tmp/mdtablefix-linux"' in result.stdout
    )
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    provisioner = (REPOSITORY_ROOT / "scripts" / "provision-mdtablefix.sh").read_text(
        encoding="utf-8"
    )
    wrapper = (REPOSITORY_ROOT / "scripts" / "mdtablefix.sh").read_text(encoding="utf-8")

    assert '"$(CARGO)" "$(MDTABLEFIX_VERSION)" "$(MDTABLEFIX_BIN_DIR)"' in makefile
    assert 'executable_suffix="${4:-}"' in provisioner
    assert 'Windows/*|*/msys*|*/cygwin*) executable_suffix=.exe' in wrapper
    assert '"$CARGO" "$MDTABLEFIX_VERSION" "$bin_dir" "$executable_suffix"' in wrapper
    assert "scripts/tests/test_markdown_formatting.py scripts/tests/test_markdown_tooling.py -q" in makefile


def test_provisions_the_pinned_release_in_an_isolated_prefix(tmp_path: Path) -> None:
    """Install only the requested binary release beneath the supplied prefix."""
    calls = tmp_path / "cargo-binstall.jsonl"
    cargo = write_executable(
        tmp_path / "cargo",
        """#!/usr/bin/env python3
import json
import os
import pathlib
import sys

arguments = sys.argv[1:]
if arguments == ["binstall", "-V"]:
    raise SystemExit(0)
with pathlib.Path(os.environ["CARGO_CALLS"]).open("a") as output:
    print(json.dumps(arguments), file=output)
install_path = pathlib.Path(arguments[arguments.index("--install-path") + 1])
install_path.mkdir(parents=True, exist_ok=True)
executable = install_path / "mdtablefix"
executable.write_text("#!/usr/bin/env bash\\necho 'mdtablefix 0.5.1'\\n")
executable.chmod(0o755)
""",
    )
    bin_dir = tmp_path / "isolated" / "mdtablefix"

    result = run_command(
        [str(PROVISION_MDTABLEFIX), str(cargo), "0.5.1", str(bin_dir), ""],
        tmp_path,
        {"CARGO_CALLS": str(calls)},
    )

    assert result.returncode == 0, result.stderr
    assert json.loads(calls.read_text(encoding="utf-8")) == [
        "binstall",
        "--no-confirm",
        "--locked",
        "--disable-strategies",
        "compile",
        "--disable-telemetry",
        "--install-path",
        str(bin_dir),
        "mdtablefix@0.5.1",
    ]
    assert (bin_dir / "mdtablefix").is_file()


def test_reuses_the_ci_prefix_and_normalises_windows_paths(tmp_path: Path) -> None:
    """Use the action's Windows prefix instead of provisioning a second binary."""
    calls = tmp_path / "cargo-binstall.jsonl"
    cargo = write_executable(
        tmp_path / "cargo",
        "#!/usr/bin/env bash\nprintf '%s\\n' invoked > \"$CARGO_CALLS\"\nexit 99\n",
    )
    write_executable(
        tmp_path / "cygpath",
        "#!/usr/bin/env bash\nprintf '%s\\n' \"$CYGPATH_RESULT\"\n",
    )
    windows_prefix = "D:\\a\\_temp\\mdtablefix-bin"
    normalized_prefix = tmp_path / "ci-prefix"
    executable = normalized_prefix / "mdtablefix.exe"
    executable.parent.mkdir()
    executable.write_text("#!/usr/bin/env bash\necho 'mdtablefix 0.5.1'\n")
    executable.chmod(0o755)

    result = run_command(
        [str(MDTABLEFIX), "--version"],
        tmp_path,
        {
            "CARGO": str(cargo),
            "CARGO_CALLS": str(calls),
            "CYGPATH_RESULT": str(normalized_prefix),
            "MDTABLEFIX_BIN_DIR": windows_prefix,
            "OSTYPE": "msys",
            "PATH": f"{tmp_path}:{os.environ['PATH']}",
            "RUNNER_OS": "Windows",
        },
    )

    assert result.returncode == 0, result.stderr
    assert result.stdout == "mdtablefix 0.5.1\n"
    assert not calls.exists(), "the preinstalled CI release must be reused"
