"""Contract tests for the pinned prover Make boundary.

These tests exercise the actual Make recipes with a fake `uv` executable. The
fake models only the child-process boundary owned by the shared package; it
does not claim to verify Rust proofs.
"""

from __future__ import annotations

import json
import os
import shlex
from pathlib import Path

import pytest

from prover_tools_fakes import (
    REPOSITORY_ROOT,
    assert_kani_check_call,
    assert_kani_install_calls,
    fake_rustup,
    fake_uv,
    kani_environment,
    read_records,
    run_make,
)

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
    """Install subprocesses prefer this Cargo home and Kani paths stay literal."""
    env, layout = kani_environment(tmp_path, explicit_cargo_home=explicit_cargo_home)

    install = run_make(
        "install-kani", env=env, variables=("KANI_INSTALL_FLAGS=--compile",)
    )
    assert install.returncode == 0, install.stderr
    assert_kani_install_calls(layout)

    check = run_make("kani-check", env=env)
    assert check.returncode == 0, check.stderr

    records = read_records(layout.capture)
    expected_home = str(layout.cargo_home) if explicit_cargo_home else None
    assert [record["cargo_home"] for record in records] == [
        expected_home,
        expected_home,
    ]
    assert_kani_check_call(layout, records)


def test_kani_check_fails_for_a_missing_selected_executable(tmp_path: Path) -> None:
    """A missing Kani binary cannot be mistaken for the pinned version."""
    env = os.environ.copy()
    uv, capture = fake_uv(tmp_path)
    env.update(
        {
            "CARGO_HOME": str(tmp_path / "empty cargo home"),
            "PATH": "/usr/bin:/bin",
            "PROVER_TOOLS_CAPTURE": str(capture),
            "UV": str(uv),
        }
    )

    result = run_make("kani-check", env=env)

    assert result.returncode != 0
    assert "cargo-kani" in result.stderr
    assert len(read_records(capture)) == 1


@pytest.mark.parametrize("target", ("kani", "kani-full", "verus"))
def test_proof_targets_fail_before_running_an_empty_inventory(
    tmp_path: Path, target: str
) -> None:
    """Empty production inventories cannot become vacuous successful targets."""
    env = os.environ.copy()
    uv, capture = fake_uv(tmp_path)
    env.update(
        {
            "PATH": "/usr/bin:/bin",
            "PROVER_TOOLS_CAPTURE": str(capture),
            "UV": str(uv),
        }
    )

    result = run_make(target, env=env)

    assert result.returncode != 0
    assert "FORMAL-EMPTY:" in result.stderr
    assert not capture.exists(), "an empty inventory must fail before starting a tool"


def test_verus_forwards_explicit_proof_files_in_order(tmp_path: Path) -> None:
    """The Make boundary preserves repeated proof-file arguments literally."""
    env = os.environ.copy()
    uv, capture = fake_uv(tmp_path)
    env.update(
        {
            "PATH": "/usr/bin:/bin",
            "PROVER_TOOLS_CAPTURE": str(capture),
            "UV": str(uv),
        }
    )
    flags = "--proof-file 'proof dir/first.rs' --proof-file \"proof's second.rs\""

    result = run_make("verus", env=env, variables=(f"VERUS_FLAGS={flags}",))

    assert result.returncode == 0, result.stderr
    arguments = read_records(capture)[0]["args"]
    first = arguments.index("--proof-file")
    second = arguments.index("--proof-file", first + 1)
    assert arguments[first + 1] == "proof dir/first.rs"
    assert arguments[second + 1] == "proof's second.rs"
    assert "--no-install-missing" in arguments
    assert "--no-ensure-toolchain" in arguments, (
        "execution must not install a toolchain; `make install-verus` owns that"
    )
    assert arguments.count("--proof-file") == 2, (
        "the boundary must not add a default proof file of its own"
    )
    assert read_records(capture)[0]["verus_z3_path"] == VERUS_BUNDLED_Z3


def _verus_install_env(tmp_path: Path) -> tuple[dict[str, str], Path, Path, Path]:
    """Return an environment, capture file, rustup log, and release manifest."""
    env = os.environ.copy()
    uv, capture = fake_uv(tmp_path)
    rustup, rustup_log = fake_rustup(tmp_path)
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

    result = run_make(
        "install-verus",
        env=env,
        variables=(f"VERUS_TOOLCHAIN_FILE={manifest}",),
    )

    assert result.returncode == 0, result.stderr
    records = read_records(capture)
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

    result = run_make(
        "install-verus",
        env=env,
        variables=(f"VERUS_TOOLCHAIN_FILE={manifest}",),
    )

    assert result.returncode != 0
    assert "VERUS-TOOLCHAIN:" in result.stderr
    assert len(read_records(capture)) == 1, "the Verus install itself still ran"
    assert not rustup_log.exists(), "no toolchain name means no rustup call"
