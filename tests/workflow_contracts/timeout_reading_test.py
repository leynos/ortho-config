"""How the timeout contract reads the files it compares.

Every assertion in ``timeout_ordering_test`` rests on turning three
files into comparable seconds. Those readings can be wrong while no file
is wrong, and this repository's own configuration cannot expose most of
the ways they can be, so they are driven with controlled values here.
"""

from __future__ import annotations

import re

import pytest
from nextest_budgets import (
    largest_test_allowance,
    seconds,
    termination_allowance,
)
from timeout_budgets import (
    CEILING_MARGIN_SECONDS,
    NEXTEST_CONFIG,
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
    required_ceiling,
)


@pytest.fixture(scope="module")
def nextest_config() -> str:
    """Return the nextest configuration file's text.

    Returns
    -------
    str
        The file's contents.
    """
    return NEXTEST_CONFIG.read_text(encoding="utf-8")


def test_the_largest_per_test_allowance_counts_the_multiplier(
    nextest_config: str,
) -> None:
    """``terminate-after`` scales the period; the budget is their product.

    This is the reading that decides every comparison above, and it is
    the one easy to get wrong: a contract reading the period alone would
    report a 120 s largest allowance where the real figure is 600 s.
    """
    largest = largest_test_allowance(nextest_config)
    periods = [
        seconds(match[1])
        for match in re.finditer(r'period\s*=\s*"([^"]+)"', nextest_config)
    ]
    assert largest > max(periods), (
        f"the largest per-test allowance came out as {largest:.0f}s, no more "
        f"than the longest bare period; terminate-after was not counted"
    )


def test_the_termination_allowance_is_the_grace_period_plus_the_margin() -> None:
    """The two terms are added, not maximized over.

    A single floor over the grace period and the margin would absorb
    every grace period below the margin, so adding a thirty-second one
    to this configuration would demand nothing more of the watchdog. No
    ``global-timeout`` is set here, so the ordering assertion that uses
    this reading is skipped entirely, which leaves this test the only
    thing standing behind it.
    """
    assert termination_allowance("") == pytest.approx(
        NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS + TERMINATION_SAFETY_MARGIN_SECONDS
    ), "an unnamed grace period must fall back to nextest's own default"
    configured = termination_allowance(
        "[profile.default]\n"
        'slow-timeout = { period = "60s", terminate-after = 1, '
        'grace-period = "30s" }\n'
    )
    assert configured == pytest.approx(30.0 + TERMINATION_SAFETY_MARGIN_SECONDS), (
        "a grace period below the margin must still raise the allowance; "
        "a maximum over the two terms would have discarded it"
    )
    largest = termination_allowance(
        "[profile.default]\n"
        'slow-timeout = { period = "60s", terminate-after = 1, '
        'grace-period = "5s" }\n'
        "\n[[profile.default.overrides]]\n"
        'slow-timeout = { period = "60s", terminate-after = 1, '
        'grace-period = "45s" }\n'
    )
    assert largest == pytest.approx(45.0 + TERMINATION_SAFETY_MARGIN_SECONDS), (
        "the largest configured grace period governs the allowance"
    )


def test_the_required_ceiling_carries_all_three_terms() -> None:
    """Watchdogs, measured work, and the margin above their sum.

    Every ceiling in this tree already sits well above its requirement,
    so dropping a term from the derivation changes nothing observable
    here and the assertion over the workflows still passes. Driving the
    derivation with controlled numbers is what makes the loss visible.
    """
    assert required_ceiling([1800.0, 1800.0], 3600.0) == pytest.approx(
        3600.0 + 3600.0 + CEILING_MARGIN_SECONDS
    ), "two watchdogs, the allowance, and the margin are all added"
    assert required_ceiling([1800.0], 0.0) == pytest.approx(
        1800.0 + CEILING_MARGIN_SECONDS
    ), "the margin applies even when nothing runs outside the watchdog"
    assert required_ceiling([], 0.0) == pytest.approx(CEILING_MARGIN_SECONDS), (
        "the margin is a term of its own, not a fraction of the others"
    )
