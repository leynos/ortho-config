"""Keep documentation subprocess deadlines inside the coverage timeout tiers."""

from __future__ import annotations

import re
import tomllib
from pathlib import Path

from nextest_budgets import budget_of, global_timeout, profile_allowance
from timeout_budgets import NEXTEST_CONFIG

SUPPORT = Path(__file__).resolve().parents[2] / "ortho_config/tests/documentation_examples"


def test_documentation_operations_leave_time_for_cleanup_and_reporting() -> None:
    """Keep documentation and BDD budgets inside their per-test ceilings."""
    policy = (SUPPORT / "process_runner_policy.rs").read_text(encoding="utf-8")
    execution_policy = (SUPPORT / "process_runner_execution_policy.rs").read_text(
        encoding="utf-8"
    )
    scenario = (SUPPORT / "scenario.rs").read_text(encoding="utf-8")
    operations = {
        kind: int(seconds)
        for kind, seconds in re.findall(
            r'Self::new\(\s*"(\w+)",\s*Duration::from_secs\((\d+)\)', policy
        )
    }
    execution_match = re.search(
        r'"execution",\s*Duration::from_secs\((\d+)\)', execution_policy
    )
    assert execution_match is not None, "execution policy must declare its timeout"
    operations["execution"] = int(execution_match[1])
    cleanup_match = re.search(
        r'Self::new\(\s*"build",\s*Duration::from_secs\(\d+\),\s*Duration::from_secs\((\d+)\)',
        policy,
    )
    scenario_match = re.search(r"PER_TEST_SECONDS:\s*u64\s*=\s*(\d+)", scenario)
    assert cleanup_match is not None, "Policy::new must declare a cleanup_timeout"
    assert scenario_match is not None, "scenario.rs must declare PER_TEST_SECONDS"
    cleanup = int(cleanup_match[1])
    per_test = int(scenario_match[1])
    nextest = NEXTEST_CONFIG.read_text(encoding="utf-8")
    assert set(operations) == {"build", "execution", "discovery"}, operations
    assert 0 < operations["discovery"] < operations["execution"] < operations["build"], (
        f"operation budgets must increase: {operations}"
    )
    assert per_test == _documentation_scenario_allowance(nextest), (
        "PER_TEST_SECONDS must equal the documentation scenario test allowance"
    )
    assert per_test > profile_allowance(nextest), (
        "documentation scenarios must use their measured per-test override"
    )
    whole_run = global_timeout(nextest)
    assert whole_run is not None, "nextest default profile must set global-timeout"
    envelope = per_test - 2 * cleanup
    assert envelope > 0, "two cleanup allowances must fit inside the per-test allowance"
    assert operations["build"] <= envelope, "build budget exceeds the effective scenario deadline"
    assert envelope + cleanup < per_test, "reporting must keep one cleanup allowance"
    assert per_test < whole_run, "per-test allowance must fit inside the whole-run budget"
    _assert_bdd_scenario_budget(nextest, cleanup)


def _documentation_scenario_allowance(nextest: str) -> float:
    """Read the shared nextest allowance for binaries using scenario.rs."""
    overrides = tomllib.loads(nextest)["profile"]["default"]["overrides"]
    binaries = (
        "documented_command",
        "documentation_examples_rust_tests",
        "identifier_artefact_e2e",
    )
    allowances: set[float] = set()
    for binary in binaries:
        matching = [
            override
            for override in overrides
            if re.search(rf"binary\({re.escape(binary)}\)", override.get("filter", ""))
        ]
        assert len(matching) == 1, f"{binary} must have one nextest timeout override"
        override = matching[0]
        assert override.get("test-group") == "orthohelp_cache", (
            f"{binary} must run in the serialized orthohelp_cache group"
        )
        assert "slow-timeout" in override, f"{binary} must have a finite test timeout"
        allowances.add(
            budget_of(
                f"profile.default.overrides.{binary}", override["slow-timeout"]
            )
        )
    assert len(allowances) == 1, "documentation scenario binaries must share one allowance"
    return allowances.pop()


def _assert_bdd_scenario_budget(nextest: str, cleanup: int) -> None:
    """Match the BDD operation deadline to its dedicated nextest allowance."""
    repository = Path(__file__).resolve().parents[2]
    bdd_source = (
        repository / "cargo-orthohelp/tests/rstest_bdd_support/behaviour/steps.rs"
    ).read_text(encoding="utf-8")
    deadline_match = re.search(r"SCENARIO_TIMEOUT_SECS:\s*u64\s*=\s*(\d+)", bdd_source)
    assert deadline_match is not None, "BDD scenarios must declare their fixed deadline"

    nextest_document = tomllib.loads(nextest)
    bdd_overrides = [
        override
        for override in nextest_document["profile"]["default"]["overrides"]
        if "binary(rstest_bdd)" in override.get("filter", "")
    ]
    assert len(bdd_overrides) == 1, "rstest_bdd must have one measured timeout override"
    bdd_override = bdd_overrides[0]
    assert bdd_override.get("test-group") == "orthohelp_cache", (
        "rstest_bdd must run in the serialized orthohelp_cache group"
    )
    bdd_allowance = budget_of(
        "profile.default.overrides.rstest_bdd", bdd_override["slow-timeout"]
    )
    bdd_envelope = int(deadline_match[1])
    whole_run = global_timeout(nextest)
    assert whole_run is not None, "nextest default profile must set global-timeout"
    assert bdd_envelope + 2 * cleanup == bdd_allowance, (
        "BDD scenario deadline must reserve cleanup and reporting inside its override"
    )
    assert bdd_envelope > profile_allowance(nextest), (
        "measured BDD envelope must use its larger override"
    )
    assert bdd_allowance < whole_run, "BDD allowance must fit inside the whole-run budget"
    assert "saturating_mul(2)" in (SUPPORT / "scenario.rs").read_text(encoding="utf-8"), (
        "documentation scenario must reserve two cleanup allowances"
    )


def test_shared_documentation_builds_start_after_peer_tests() -> None:
    """Keep direct Cargo and nextest test scheduling ahead of scenario deadlines."""
    repository = Path(__file__).resolve().parents[2]
    nextest = tomllib.loads(NEXTEST_CONFIG.read_text(encoding="utf-8"))
    groups = nextest["test-groups"]
    assert groups["orthohelp_cache"]["max-threads"] == 1, (
        "orthohelp_cache must allow exactly one concurrent test"
    )

    cache_filters = [
        override["filter"]
        for override in nextest["profile"]["default"]["overrides"]
        if override.get("test-group") == "orthohelp_cache"
    ]
    assert cache_filters, "shared-cache binaries need a nextest group"
    required_binaries = {
        "rstest_bdd",
        "powershell_windows",
        "nested_subcommand_end_to_end",
        "documented_command",
        "documentation_examples_rust_tests",
        "agent_context_contract",
        "policy_report_contract",
        "golden_tests",
        "identifier_artefact_e2e",
    }
    missing = {
        binary
        for binary in required_binaries
        if not any(f"binary({binary})" in cache_filter for cache_filter in cache_filters)
    }
    assert not missing, f"shared documentation cache users escaped serialization: {missing}"

    direct_tests = (
        repository / "ortho_config/tests/documentation_examples_rust_tests.rs"
    ).read_text(encoding="utf-8")
    workspace_tests = (
        repository / "ortho_config/tests/documentation_examples/workspace_tests.rs"
    ).read_text(encoding="utf-8")
    for name in (
        "documented_rust_compiles_and_runs",
        "documented_cargo_external_subcommand_parses_both_invocation_forms",
        "aliased_dependency_example_compiles_and_runs",
    ):
        assert re.search(
            rf"#\[serial_test::serial\(documentation_build\)\]\s*fn {name}\b",
            direct_tests,
        ), f"direct cargo test must serialize {name} before its deadline starts"
    assert re.search(
        r"#\[serial_test::serial\(documentation_build\)\]\s*fn "
        r"independently_owned_workspaces_support_concurrent_interleavings\b",
        workspace_tests,
    ), "the concurrent-workspace regression must not compete with other build tests"
