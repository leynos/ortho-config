"""Contract tests for the binary-only Whitaker installer in Linux CI.

The released archive is installed directly because cargo-binstall 1.19.1
rejected the archive member that its own error named, even though that member
exists. The shell helper verifies the GitHub API digests and the release
sidecar before extracting that exact member. These tests pin the workflow
boundary; ``scripts/tests/test_install_whitaker_binary.py`` drives the helper
against a local release fixture.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

import pytest
import yaml

WORKFLOW_PATH: typ.Final[Path] = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"
)
STEP_NAME: typ.Final[str] = "Install Whitaker"
TOKEN_CARRIER: typ.Final[str] = "INSTALL_TOKEN"
TOKEN_EXPRESSION: typ.Final[str] = "${{ github.token }}"
TOKEN_NAMES: typ.Final[frozenset[str]] = frozenset({"GH_TOKEN", "GITHUB_TOKEN"})
HELPER_PATH: typ.Final[str] = "scripts/install_whitaker_binary.sh"
CACHE_PATH: typ.Final[str] = "~/.cache/ortho-config/whitaker-installer"
DYLINT_LINK_PACKAGE: typ.Final[str] = "dylint-link@6.0.1"


@pytest.fixture(scope="module")
def ci_workflow() -> dict[str, typ.Any]:
    """Return the parsed CI workflow."""
    document = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    assert isinstance(document, dict), f"{WORKFLOW_PATH.name} must parse as a mapping"
    return document


@pytest.fixture(scope="module")
def install_step(ci_workflow: dict[str, typ.Any]) -> dict[str, typ.Any]:
    """Return the unique Whitaker installation step."""
    jobs = ci_workflow.get("jobs")
    assert isinstance(jobs, dict), f"{WORKFLOW_PATH.name} must declare jobs"
    matches = [
        step
        for job in jobs.values()
        if isinstance(job, dict)
        for step in job.get("steps") or []
        if isinstance(step, dict) and step.get("name") == STEP_NAME
    ]
    assert len(matches) == 1, f"expected one {STEP_NAME!r} step, found {len(matches)}"
    return matches[0]


def _joined_shell_command(script: str, package: str) -> str:
    """Return the continued shell command that installs exactly one package."""
    lines = script.splitlines()
    matches = [index for index, line in enumerate(lines) if package in line]
    assert len(matches) == 1, f"expected one command for {package!r}, found {len(matches)}"
    start = matches[0]
    while start > 0 and lines[start - 1].rstrip().endswith("\\"):
        start -= 1
    end = matches[0]
    while end < len(lines) - 1 and lines[end].rstrip().endswith("\\"):
        end += 1
    return " ".join(line.strip().removesuffix("\\").rstrip() for line in lines[start : end + 1])


def test_the_step_carries_the_token_by_value(install_step: dict[str, typ.Any]) -> None:
    """The helper receives the workflow token under its inert carrier name."""
    environment = install_step.get("env") or {}
    assert environment.get(TOKEN_CARRIER) == TOKEN_EXPRESSION, (
        f"{STEP_NAME} must set {TOKEN_CARRIER} to the workflow token; "
        f"it sets {environment.get(TOKEN_CARRIER)!r}"
    )


@pytest.mark.parametrize("token_name", sorted(TOKEN_NAMES))
def test_the_step_does_not_export_a_tool_token(
    install_step: dict[str, typ.Any], token_name: str
) -> None:
    """Only commands that contact GitHub receive a tool-recognized token."""
    environment = install_step.get("env") or {}
    assert token_name not in environment, (
        f"{STEP_NAME} must not export {token_name} to the helper or installed binary"
    )


def test_the_step_uses_the_verified_release_helper(
    install_step: dict[str, typ.Any],
) -> None:
    """The installer is direct and cannot fall back to a source build."""
    script = str(install_step.get("run", ""))
    assert HELPER_PATH in script, f"{STEP_NAME} must run {HELPER_PATH}"
    assert "cargo install" not in script, f"{STEP_NAME} must not build Whitaker from source"
    assert script.count("cargo binstall") == 1, (
        f"{STEP_NAME} may use cargo-binstall only for dylint-link: {script!r}"
    )


def test_the_release_asset_cache_is_verified(ci_workflow: dict[str, typ.Any]) -> None:
    """The cache holds inputs that the helper verifies, never a bare binary."""
    steps = ci_workflow["jobs"]["build-test"]["steps"]
    cache_step = next(step for step in steps if step.get("name") == "Cache Whitaker installer")
    cache_path = cache_step["with"]["path"]
    assert cache_path == CACHE_PATH, (
        f"Whitaker cache must hold verified release assets: {cache_path!r}"
    )


def test_dylint_link_remains_authenticated_and_binary_only(
    install_step: dict[str, typ.Any],
) -> None:
    """The direct installer does not weaken the existing Dylint protection."""
    script = str(install_step.get("run", ""))
    dylint_command = _joined_shell_command(script, DYLINT_LINK_PACKAGE)
    assert 'GH_TOKEN="${INSTALL_TOKEN}"' in dylint_command, (
        "dylint-link must receive GH_TOKEN locally"
    )
    assert 'GITHUB_TOKEN="${INSTALL_TOKEN}"' in dylint_command, (
        "dylint-link must receive GITHUB_TOKEN locally"
    )
    assert "--disable-strategies compile" in dylint_command, (
        "dylint-link must not fall back to a source build"
    )
    assert DYLINT_LINK_PACKAGE in dylint_command, "the Dylint preinstall must name its package"
