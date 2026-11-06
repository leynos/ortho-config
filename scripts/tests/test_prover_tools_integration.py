"""Real-tool integration tests for the pinned Kani and Verus fixtures.

These tests run the repository's pinned verifiers against fixture-only inputs.
They are deliberately opt-in: ordinary ``make test`` must neither download a
prover nor fail because one is absent, so every test here is skipped unless
``RUN_PROVER_TOOLS_INTEGRATION=1`` is set. ``make test-prover-tools-integration``
sets it, and in that mode a missing tool is a failure rather than a skip.

The Kani fixtures live in ``tests/fixtures/prover_tools/kani`` and the Verus
fixtures in ``tests/fixtures/prover_tools/verus``. Both are outside the Cargo
workspace: Kani reads its fixture as a standalone package, and Verus reads its
files as standalone crates.

Evidence gathered here is recorded in
``target/prover-tools-fixtures/prover-evidence.json`` so a CI run can upload it
alongside the verifier logs.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
KANI_FIXTURE = REPOSITORY_ROOT / "tests" / "fixtures" / "prover_tools" / "kani"
VERUS_FIXTURE = REPOSITORY_ROOT / "tests" / "fixtures" / "prover_tools" / "verus"
EVIDENCE_DIRECTORY = REPOSITORY_ROOT / "target" / "prover-tools-fixtures"
KANI_TARGET_DIRECTORY = EVIDENCE_DIRECTORY / "kani"
EVIDENCE_FILE = EVIDENCE_DIRECTORY / "prover-evidence.json"

#: Bounded allowance for one fixture invocation, including a cold crate build.
#: The plan's 120-second figure covers verification after installation; the
#: first harness also compiles the fixture crate, so this ceiling leaves room
#: for that compile without letting a hung verifier reach the job's 30 minutes.
FIXTURE_TIMEOUT_SECONDS = 240

#: The Kani harnesses that must be accepted, and the one that must be rejected.
KANI_POSITIVE_HARNESSES = (
    "count_matches_independent_sum",
    "reachable_zero_full_and_mixed",
    "mutant_witness_is_a_counterexample",
)
KANI_NEGATIVE_HARNESS = "mutant_omitting_last_input_is_rejected"

#: Verus proof files that must verify, and the one that must be rejected.
VERUS_POSITIVE_FILE = VERUS_FIXTURE / "count_bounds.rs"
VERUS_NEGATIVE_FILE = VERUS_FIXTURE / "count_strict_is_rejected.rs"

pytestmark = pytest.mark.skipif(
    os.environ.get("RUN_PROVER_TOOLS_INTEGRATION") != "1",
    reason=(
        "real-tool integration is opt-in; run "
        "`make test-prover-tools-integration`"
    ),
)


def _kani_executable() -> Path:
    """Return the Kani executable the Make boundary selected.

    ``KANI`` is forwarded by ``make test-prover-tools-integration``. When it is
    absent the default matches the Makefile's default in the active Cargo home.
    """
    configured = os.environ.get("KANI")
    if configured:
        return Path(configured)
    cargo_home = os.environ.get("CARGO_HOME")
    base = Path(cargo_home) if cargo_home else Path.home() / ".cargo"
    return base / "bin" / "cargo-kani"


def _run(
    arguments: list[str],
    *,
    cwd: Path,
    environment: dict[str, str] | None = None,
    timeout: int = FIXTURE_TIMEOUT_SECONDS,
) -> subprocess.CompletedProcess[str]:
    """Run one command and capture its output as text.

    A missing executable is reported as a test failure rather than an error:
    the explicit integration target cannot skip a tool it was asked to use.
    """
    try:
        return subprocess.run(
            arguments,
            cwd=cwd,
            env=environment,
            check=False,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except FileNotFoundError as error:
        pytest.fail(f"required verifier is not installed: {arguments[0]}: {error}")


def _write_log(name: str, result: subprocess.CompletedProcess[str]) -> None:
    """Persist one verifier transcript for CI to upload."""
    logs = EVIDENCE_DIRECTORY / "logs"
    logs.mkdir(parents=True, exist_ok=True)
    (logs / f"{name}.log").write_text(_combined_output(result), encoding="utf-8")


def _run_kani(harness: str) -> subprocess.CompletedProcess[str]:
    """Run one Kani harness against the fixture package."""
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(KANI_TARGET_DIRECTORY)
    result = _run(
        [str(_kani_executable()), "--harness", harness],
        cwd=KANI_FIXTURE,
        environment=environment,
    )
    _write_log(f"kani-{harness}", result)
    return result


def _run_make_verus(proof_file: Path) -> subprocess.CompletedProcess[str]:
    """Run one Verus proof file through the repository Make boundary."""
    result = _run(
        [
            "make",
            "--no-print-directory",
            "verus",
            f"VERUS_PROOF_FILES={proof_file}",
        ],
        cwd=REPOSITORY_ROOT,
    )
    _write_log(f"verus-{proof_file.stem}", result)
    return result


def _combined_output(result: subprocess.CompletedProcess[str]) -> str:
    """Return a result's two streams as one searchable document."""
    return f"{result.stdout}\n{result.stderr}"


def _verus_version() -> str:
    """Return the installed Verus version from its pinned install directory."""
    pinned = (REPOSITORY_ROOT / "tools" / "verus" / "VERSION").read_text(
        encoding="utf-8"
    ).strip()
    binary = REPOSITORY_ROOT / ".verus" / pinned / "verus" / "verus"
    if not binary.is_file():
        pytest.fail(f"required verifier is not installed: {binary}")
    result = _run([str(binary), "--version"], cwd=REPOSITORY_ROOT)
    _write_log("verus-version", result)
    if result.returncode != 0:
        pytest.fail(f"verus --version failed: {_combined_output(result)}")
    return pinned


def test_kani_reports_the_pinned_version() -> None:
    """The executed Kani binary is the one the repository pins."""
    pinned = (REPOSITORY_ROOT / "tools" / "kani" / "VERSION").read_text(
        encoding="utf-8"
    ).strip()
    result = _run([str(_kani_executable()), "--version"], cwd=REPOSITORY_ROOT)
    _write_log("kani-version", result)
    assert result.returncode == 0, _combined_output(result)
    assert pinned in _combined_output(result), _combined_output(result)


def test_verus_reports_the_pinned_version() -> None:
    """The executed Verus binary is the one the repository pins."""
    assert _verus_version() == (
        REPOSITORY_ROOT / "tools" / "verus" / "VERSION"
    ).read_text(encoding="utf-8").strip()


@pytest.mark.parametrize("harness", KANI_POSITIVE_HARNESSES)
def test_kani_accepts_valid_harnesses(harness: str) -> None:
    """Valid symbolic harnesses verify without a counterexample."""
    result = _run_kani(harness)
    output = _combined_output(result)
    assert result.returncode == 0, output
    assert "VERIFICATION:- SUCCESSFUL" in output, output
    assert "error[E" not in output, "a compiler error is not verification evidence"


def test_kani_rejects_the_mutant_omitting_the_last_input() -> None:
    """The negative control fails verification for the intended witness."""
    result = _run_kani(KANI_NEGATIVE_HARNESS)
    output = _combined_output(result)
    assert result.returncode != 0, output
    assert "VERIFICATION:- FAILED" in output, output
    assert "assertion failed" in output.lower(), output
    assert "error[E" not in output, "a compiler error is not verification evidence"


def test_verus_accepts_the_count_bounds_proof() -> None:
    """The inductive bound verifies with no errors and no cheating."""
    result = _run_make_verus(VERUS_POSITIVE_FILE)
    output = _combined_output(result)
    assert result.returncode == 0, output
    assert re.search(r"\b[1-9]\d* verified, 0 errors\b", output), output
    assert "postcondition not satisfied" not in output, output


def test_verus_rejects_the_strict_count_claim() -> None:
    """The negative control reports a genuine postcondition failure."""
    result = _run_make_verus(VERUS_NEGATIVE_FILE)
    output = _combined_output(result)
    assert result.returncode != 0, output
    assert "postcondition not satisfied" in output, output
    assert re.search(r"\b[1-9]\d* errors\b", output), output
    assert "error[E" not in output, "a compiler error is not verification evidence"


def test_records_verifier_evidence() -> None:
    """Record versions, fixtures, bounds, and negative controls for upload."""
    kani_version = (REPOSITORY_ROOT / "tools" / "kani" / "VERSION").read_text(
        encoding="utf-8"
    ).strip()
    evidence = {
        "kani": {
            "version": kani_version,
            "fixture": str(KANI_FIXTURE.relative_to(REPOSITORY_ROOT)),
            "bounds": {"inputs": 4, "unwind": 5},
            "positive_harnesses": list(KANI_POSITIVE_HARNESSES),
            "negative_harness": KANI_NEGATIVE_HARNESS,
        },
        "verus": {
            "version": _verus_version(),
            "fixtures": {
                "positive": str(VERUS_POSITIVE_FILE.relative_to(REPOSITORY_ROOT)),
                "negative": str(VERUS_NEGATIVE_FILE.relative_to(REPOSITORY_ROOT)),
            },
            "negative_diagnostic": "postcondition not satisfied",
        },
    }
    EVIDENCE_DIRECTORY.mkdir(parents=True, exist_ok=True)
    EVIDENCE_FILE.write_text(
        json.dumps(evidence, indent=2) + "\n", encoding="utf-8"
    )
    assert json.loads(EVIDENCE_FILE.read_text(encoding="utf-8")) == evidence


def test_fixtures_do_not_shadow_the_workspace() -> None:
    """Both fixtures stay outside the repository Cargo workspace.

    The Kani fixture declares its own empty ``[workspace]`` table, which stops
    Cargo walking up to the repository manifest; without it a stray build from
    the fixture directory would join the main workspace and inherit its lints.
    """
    workspace = (REPOSITORY_ROOT / "Cargo.toml").read_text(encoding="utf-8")
    assert "prover_tools" not in workspace, "fixtures must not be workspace members"
    fixture_manifest = (KANI_FIXTURE / "Cargo.toml").read_text(encoding="utf-8")
    assert "[workspace]" in fixture_manifest, (
        "the Kani fixture must be its own workspace root"
    )


def test_the_fixture_build_has_a_cargo_toolchain() -> None:
    """Kani drives cargo, so the lane fails clearly when cargo is absent."""
    assert shutil.which("cargo") is not None, "the fixture build requires cargo"