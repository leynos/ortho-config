"""Keep documentation subprocess deadlines inside the coverage timeout tiers."""

from __future__ import annotations

import re
from pathlib import Path

from nextest_budgets import global_timeout, profile_allowance
from timeout_budgets import NEXTEST_CONFIG

SUPPORT = Path(__file__).resolve().parents[2] / "ortho_config/tests/documentation_examples"


def test_documentation_operations_leave_time_for_cleanup_and_reporting() -> None:
    """Fit this binary's default allowance, not the larger trybuild override."""
    policy = (SUPPORT / "process_runner_policy.rs").read_text(encoding="utf-8")
    scenario = (SUPPORT / "scenario.rs").read_text(encoding="utf-8")
    operations = {
        kind: int(seconds)
        for kind, seconds in re.findall(
            r"Self::new\(Kind::(\w+),\s*Duration::from_secs\((\d+)\)\)", policy
        )
    }
    cleanup_match = re.search(
        r"cleanup_timeout:\s*Duration::from_secs\((\d+)\)", policy
    )
    scenario_match = re.search(r"PER_TEST_SECONDS:\s*u64\s*=\s*(\d+)", scenario)
    assert cleanup_match is not None
    assert scenario_match is not None
    cleanup = int(cleanup_match[1])
    per_test = int(scenario_match[1])
    nextest = NEXTEST_CONFIG.read_text(encoding="utf-8")
    assert set(operations) == {"Build", "Execution", "Discovery"}
    assert 0 < operations["Discovery"] < operations["Execution"] < operations["Build"]
    assert per_test == profile_allowance(nextest)
    assert operations["Build"] <= per_test
    whole_run = global_timeout(nextest)
    assert whole_run is not None
    assert 0 < per_test - cleanup * 2 + cleanup < per_test < whole_run
    assert "saturating_mul(2)" in scenario
