"""Contract tests for the pinned prover Make boundary.

These tests exercise the actual Make recipes with a fake `uv` executable. The
fake models only the child-process boundary owned by the shared package; it
does not claim to verify Rust proofs.
"""

from __future__ import annotations

import json
import os
import shlex
import subprocess
import textwrap
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
PROVER_TOOLS_SOURCE = "592558f7183b554fc749696246984c5d7300d029"
VERUS_DIGEST = (
    "0f693297869f8804c08c1726c8b4e80cacb97741f66380ffdf5f5885dabec376"
)
VERUS_VERSION = (
    REPOSITORY_ROOT / "tools/verus/VERSION"
).read_text(encoding="utf-8").strip()
#: Verus refuses a solver other than the one its release shipped with, so the
#: Make boundary points at the bundled Z3 rather than a separately installed one.
VERUS_BUNDLED_Z3 = f"{REPOSITORY_ROOT}/.verus/{VERUS_VERSION}/verus/z3"


def _write_executable(path: Path, content: str) -> Path:
    """Write an executable fixture script and return its path."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(textwrap.dedent(content).lstrip(), encoding="utf-8")
    path.chmod(0o755)
    return path


def _fake_uv(tmp_path: Path) -> tuple[Path, Path]:
    """Create a `uv` stand-in that captures argv and exercises package parsing."""
    capture = tmp_path / "prover-tools-calls.jsonl"
    uv = _write_executable(
        tmp_path / "bin" / "uv",
        """
        #!/usr/bin/env python3
        import json
        import os
        import shlex
        import subprocess
        import sys
        from pathlib import Path

        record = {
            "args": sys.argv[1:],
            "cargo_home": os.environ.get("CARGO_HOME"),
            "kani": os.environ.get("KANI"),
            "path": os.environ.get("PATH"),
            "verus_z3_path": os.environ.get("VERUS_Z3_PATH"),
        }
        with Path(os.environ["PROVER_TOOLS_CAPTURE"]).open("a", encoding="utf-8") as output:
            output.write(json.dumps(record) + "\\n")

        if "kani" in sys.argv and "install" in sys.argv:
            if "--compile" in sys.argv:
                subprocess.run(
                    ["cargo", "install", "--locked", "kani-verifier", "--version", "0.67.0"],
                    check=True,
                )
            subprocess.run(["cargo", "kani", "setup"], check=True)
        if "kani" in sys.argv and "check-version" in sys.argv:
            command_value = sys.argv[sys.argv.index("--kani-command") + 1]
            command = shlex.split(command_value)
            subprocess.run([*command, "--version"], check=True)
        """,
    )
    return uv, capture


def _fake_rustup(tmp_path: Path) -> tuple[Path, Path]:
    """Create a `rustup` stand-in that records the toolchain it is asked for."""
    log = tmp_path / "rustup-invocations.jsonl"
    rustup = _write_executable(
        tmp_path / "bin" / "rustup",
        f"""
        #!/usr/bin/env python3
        import json
        import sys
        from pathlib import Path
        with Path({str(log)!r}).open("a", encoding="utf-8") as output:
            output.write(json.dumps({{"args": sys.argv[1:]}}) + "\\n")
        """,
    )
    return rustup, log


def _records(capture: Path) -> list[dict[str, object]]:
    """Return captured fake-package invocations in call order."""
    captured = capture.read_text(encoding="utf-8").splitlines()
    return [json.loads(line) for line in captured]


def _run_make(
    target: str,
    *,
    env: dict[str, str],
    variables: tuple[str, ...] = (),
) -> subprocess.CompletedProcess[str]:
    """Run one repository Make target with caller-supplied environment and flags."""
    return subprocess.run(
        ["make", "--no-print-directory", *variables, target],
        cwd=REPOSITORY_ROOT,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )


def test_pinned_versions_and_shared_tool_revision_are_declared() -> None:
    """The Make boundary consumes immutable repository-owned tool pins."""
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")

    assert (
        REPOSITORY_ROOT / "tools/kani/VERSION"
    ).read_text(encoding="utf-8").strip() == "0.67.0"
    assert (
        REPOSITORY_ROOT / "tools/verus/VERSION"
    ).read_text(encoding="utf-8").strip() == "0.2026.01.30.44ebdee"
    assert (
        REPOSITORY_ROOT / "tools/verus/SHA256SUMS"
    ).read_text(encoding="utf-8").strip() == (
        f"{VERUS_DIGEST}  verus-0.2026.01.30.44ebdee-x86-linux.zip"
    )
    assert f"PROVER_TOOLS_SOURCE ?= {PROVER_TOOLS_SOURCE}" in makefile
    assert "--python 3.14" in makefile


@pytest.mark.parametrize("explicit_cargo_home", (False, True))
def test_kani_install_and_version_check_preserve_cargo_home(
    tmp_path: Path, explicit_cargo_home: bool
) -> None:
    """Install subprocesses prefer this Cargo home and Kani paths stay literal.

    A conflicting `cargo` earlier on PATH reproduces the donor installer bug.
    The custom home includes spaces and an apostrophe to exercise both shell
    quoting and the shared package's `shlex.split` command boundary.
    """
    home = tmp_path / "home dir o'brien"
    cargo_home = (
        tmp_path / "cargo home o'brien" if explicit_cargo_home else home / ".cargo"
    )
    cargo_bin = cargo_home / "bin"
    conflict_bin = tmp_path / "tools prefix" / "bin"
    cargo_log = tmp_path / "cargo-invocations.jsonl"
    kani_log = tmp_path / "kani-invocations.jsonl"
    original_path = os.pathsep.join((str(conflict_bin), "/usr/bin", "/bin"))
    env = os.environ.copy()
    uv, capture = _fake_uv(tmp_path)
    env.update(
        {
            "CARGO_HOME": str(cargo_home),
            "HOME": str(home),
            "PATH": original_path,
            "PROVER_TOOLS_CAPTURE": str(capture),
            "UV": str(uv),
        }
    )
    if not explicit_cargo_home:
        env.pop("CARGO_HOME")
    env.pop("KANI", None)
    _write_executable(
        cargo_bin / "cargo",
        f"""
        #!/usr/bin/env python3
        import json
        import sys
        from pathlib import Path
        with Path({str(cargo_log)!r}).open("a", encoding="utf-8") as output:
            output.write(json.dumps({{"path": sys.argv[0], "args": sys.argv[1:]}}) + "\\n")
        """,
    )
    _write_executable(
        conflict_bin / "cargo",
        """
        #!/usr/bin/env python3
        import sys
        raise SystemExit("conflicting cargo selected: " + sys.argv[0])
        """,
    )
    _write_executable(
        cargo_bin / "cargo-kani",
        f"""
        #!/usr/bin/env python3
        import json
        import sys
        from pathlib import Path
        with Path({str(kani_log)!r}).open("a", encoding="utf-8") as output:
            output.write(json.dumps({{"path": sys.argv[0], "args": sys.argv[1:]}}) + "\\n")
        print("Kani 0.67.0")
        """,
    )

    install = _run_make(
        "install-kani", env=env, variables=("KANI_INSTALL_FLAGS=--compile",)
    )
    assert install.returncode == 0, install.stderr
    cargo_invocations = [
        json.loads(line) for line in cargo_log.read_text(encoding="utf-8").splitlines()
    ]
    assert cargo_invocations == [
        {
            "path": str(cargo_bin / "cargo"),
            "args": ["install", "--locked", "kani-verifier", "--version", "0.67.0"],
        },
        {"path": str(cargo_bin / "cargo"), "args": ["kani", "setup"]},
    ]

    check = _run_make("kani-check", env=env)
    assert check.returncode == 0, check.stderr
    kani_invocation = json.loads(kani_log.read_text(encoding="utf-8").strip())
    assert kani_invocation == {
        "path": str(cargo_bin / "cargo-kani"),
        "args": ["--version"],
    }

    records = _records(capture)
    assert [record["cargo_home"] for record in records] == [
        str(cargo_home) if explicit_cargo_home else None,
        str(cargo_home) if explicit_cargo_home else None,
    ]
    check_arguments = records[1]["args"]
    command_value = check_arguments[check_arguments.index("--kani-command") + 1]
    assert shlex.split(command_value) == [str(cargo_bin / "cargo-kani")]
    assert str(REPOSITORY_ROOT) in check_arguments
    assert records[0]["path"].split(os.pathsep) == [
        str(cargo_bin),
        str(conflict_bin),
        "/usr/bin",
        "/bin",
    ]


def test_kani_check_fails_for_a_missing_selected_executable(tmp_path: Path) -> None:
    """A missing Kani binary cannot be mistaken for the pinned version."""
    env = os.environ.copy()
    uv, capture = _fake_uv(tmp_path)
    env.update(
        {
            "CARGO_HOME": str(tmp_path / "empty cargo home"),
            "PATH": "/usr/bin:/bin",
            "PROVER_TOOLS_CAPTURE": str(capture),
            "UV": str(uv),
        }
    )

    result = _run_make("kani-check", env=env)

    assert result.returncode != 0
    assert "cargo-kani" in result.stderr
    assert len(_records(capture)) == 1


@pytest.mark.parametrize("target", ("kani", "kani-full", "verus"))
def test_proof_targets_fail_before_running_an_empty_inventory(
    tmp_path: Path, target: str
) -> None:
    """Empty production inventories cannot become vacuous successful targets."""
    env = os.environ.copy()
    uv, capture = _fake_uv(tmp_path)
    env.update(
        {
            "PATH": "/usr/bin:/bin",
            "PROVER_TOOLS_CAPTURE": str(capture),
            "UV": str(uv),
        }
    )

    result = _run_make(target, env=env)

    assert result.returncode != 0
    assert "FORMAL-EMPTY:" in result.stderr
    assert not capture.exists(), "an empty inventory must fail before starting a tool"


def test_verus_forwards_explicit_proof_files_in_order(tmp_path: Path) -> None:
    """The Make boundary preserves repeated proof-file arguments literally."""
    env = os.environ.copy()
    uv, capture = _fake_uv(tmp_path)
    env.update(
        {
            "PATH": "/usr/bin:/bin",
            "PROVER_TOOLS_CAPTURE": str(capture),
            "UV": str(uv),
        }
    )
    flags = "--proof-file 'proof dir/first.rs' --proof-file \"proof's second.rs\""

    result = _run_make("verus", env=env, variables=(f"VERUS_FLAGS={flags}",))

    assert result.returncode == 0, result.stderr
    arguments = _records(capture)[0]["args"]
    first = arguments.index("--proof-file")
    second = arguments.index("--proof-file", first + 1)
    assert arguments[first + 1] == "proof dir/first.rs"
    assert arguments[second + 1] == "proof's second.rs"
    assert "--no-install-missing" in arguments
    assert arguments.count("--proof-file") == 2, (
        "the boundary must not add a default proof file of its own"
    )
    assert _records(capture)[0]["verus_z3_path"] == VERUS_BUNDLED_Z3


def _verus_install_env(tmp_path: Path) -> tuple[dict[str, str], Path, Path, Path]:
    """Return an environment, capture file, rustup log, and release manifest."""
    env = os.environ.copy()
    uv, capture = _fake_uv(tmp_path)
    rustup, rustup_log = _fake_rustup(tmp_path)
    manifest = tmp_path / "version.json"
    env.update(
        {
            "PATH": os.pathsep.join((str(rustup.parent), "/usr/bin", "/bin")),
            "PROVER_TOOLS_CAPTURE": str(capture),
            "UV": str(uv),
        }
    )
    return env, capture, rustup_log, manifest


def test_verus_installation_uses_the_bundled_solver_and_required_toolchain(
    tmp_path: Path,
) -> None:
    """Verus installs no separate solver and the Rust toolchain its release needs.

    The release refuses to run under any other toolchain, and a CI runner only
    has the repository's own pin, so installation must supply the one the
    release names in its manifest.
    """
    env, capture, rustup_log, manifest = _verus_install_env(tmp_path)
    manifest.write_text(
        json.dumps({"verus": {"toolchain": "1.93.0-x86_64-unknown-linux-gnu"}}),
        encoding="utf-8",
    )

    result = _run_make(
        "install-verus",
        env=env,
        variables=(f"VERUS_TOOLCHAIN_FILE={manifest}",),
    )

    assert result.returncode == 0, result.stderr
    records = _records(capture)
    assert len(records) == 1
    assert records[0]["args"][-4:] == [
        "verus",
        "install",
        "--repo-root",
        str(REPOSITORY_ROOT),
    ]
    assert "z3" not in records[0]["args"], (
        "a separate Z3 download would not match the Verus release"
    )
    rustup_calls = [
        json.loads(line)["args"]
        for line in rustup_log.read_text(encoding="utf-8").splitlines()
    ]
    assert rustup_calls == [
        [
            "toolchain",
            "install",
            "1.93.0-x86_64-unknown-linux-gnu",
            "--profile",
            "minimal",
        ]
    ]


def test_verus_installation_fails_when_the_release_names_no_toolchain(
    tmp_path: Path,
) -> None:
    """An unreadable release manifest cannot silently skip the toolchain."""
    env, capture, rustup_log, manifest = _verus_install_env(tmp_path)
    manifest.write_text("{}", encoding="utf-8")

    result = _run_make(
        "install-verus",
        env=env,
        variables=(f"VERUS_TOOLCHAIN_FILE={manifest}",),
    )

    assert result.returncode != 0
    assert "VERUS-TOOLCHAIN:" in result.stderr
    assert len(_records(capture)) == 1, "the Verus install itself still ran"
    assert not rustup_log.exists(), "no toolchain name means no rustup call"
