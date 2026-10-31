"""Contract test for the development build standard (``rust-build-defaults``).

The Concordat rule needs the ``mold`` linker as a Linux-only default in
``.cargo/config.toml``, and needs every gate recipe that assigns ``RUSTFLAGS``
to restate it, because an assigned ``RUSTFLAGS`` replaces every ``rustflags``
source. The jobs that build under that configuration have to install ``mold``.
The Whitaker route is the exception: its installer-managed toolchain must not
inherit the flag.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import os
import subprocess
import tomllib
import typing as typ
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]
MOLD: typ.Final[str] = "-Clink-arg=-fuse-ld=mold"
LINUX_TABLE: typ.Final[str] = 'cfg(target_os = "linux")'
GATES: typ.Final[tuple[str, ...]] = ("test", "boundary", "typecheck")


def _dry_run(target: str, *overrides: str) -> str:
    """Return normalized output from a dry-run of one Make target."""
    output = subprocess.run(
        ["make", "--no-print-directory", "--dry-run", target, *overrides],
        cwd=ROOT,
        env=os.environ.copy(),
        capture_output=True,
        check=True,
        text=True,
    ).stdout
    return " ".join(output.replace("\\\n\t", " ").split())


def _workflow(name: str) -> dict[str, typ.Any]:
    """Parse one workflow file."""
    text = (ROOT / ".github" / "workflows" / name).read_text("utf-8")
    return yaml.safe_load(text)


def test_mold_is_a_linux_only_default() -> None:
    """The linker flag lives in the Linux table and in no platform-wide source."""
    config = tomllib.loads((ROOT / ".cargo" / "config.toml").read_text("utf-8"))
    assert MOLD in config["target"][LINUX_TABLE]["rustflags"], (
        "the Linux target table must select mold"
    )
    assert MOLD not in config.get("build", {}).get("rustflags", []), (
        "mold ships for Linux only, so [build] must not name it"
    )
    assert not config.get("profile"), (
        "the stable pin cannot select a codegen backend, so no profile may"
    )


@pytest.mark.parametrize("target", GATES)
@pytest.mark.parametrize(
    ("host_os", "expects_mold"),
    [("Linux", True), ("Darwin", False), ("Windows_NT", False)],
)
def test_gate_recipes_restate_mold_on_linux_only(
    target: str, host_os: str, expects_mold: bool
) -> None:
    """The gates add mold to the warning policy on Linux hosts only."""
    command = _dry_run(target, f"BUILD_HOST_OS={host_os}")
    assert 'RUSTFLAGS="-D warnings' in command, (
        f"make {target} must keep denying compiler warnings"
    )
    assert (MOLD in command) is expects_mold, (
        f"make {target} on {host_os} must {'add' if expects_mold else 'omit'} mold"
    )


def test_whitaker_route_takes_no_development_flags() -> None:
    """The Whitaker recipe assigns the warning policy alone."""
    command = _dry_run("lint-whitaker", "BUILD_HOST_OS=Linux")
    assert MOLD not in command, "the Whitaker route must not inherit mold"


def test_build_test_job_installs_mold() -> None:
    """The job running the gates installs mold through setup-rust."""
    steps = _workflow("ci.yml")["jobs"]["build-test"]["steps"]
    setup = [s for s in steps if "/setup-rust@" in str(s.get("uses"))]
    assert len(setup) == 1, "build-test must have exactly one setup-rust step"
    assert (setup[0].get("with") or {}).get("install-mold") == "true", (
        "build-test must install the mold linker the configuration selects"
    )


def test_beta_clippy_job_installs_mold_on_linux() -> None:
    """The advisory Clippy job has no RUSTFLAGS, so the configuration applies."""
    steps = _workflow("ci.yml")["jobs"]["clippy-beta"]["steps"]
    install = [s for s in steps if "mold" in str(s.get("run", ""))]
    assert len(install) == 1, "clippy-beta must install mold exactly once"
    assert "linux" in str(install[0].get("if")), "the install must be Linux-only"


def test_mutation_lane_installs_mold() -> None:
    """The reusable mutation workflow is told to install mold."""
    job = _workflow("mutation-testing.yml")["jobs"]["mutation"]
    assert job["with"].get("install-mold") == "true", (
        "the mutation lane builds under .cargo/config.toml and needs mold"
    )
