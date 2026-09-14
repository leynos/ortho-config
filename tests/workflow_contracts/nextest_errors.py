"""The faults a nextest configuration can carry.

Separated from ``nextest_budgets`` so the duration grammar and the
configuration reading can both raise them without either importing the
other, and so neither module outgrows the 400-line limit ``AGENTS.md``
sets.
"""

from __future__ import annotations


class TimeoutBudgetError(ValueError):
    """Raised when a configured budget cannot be read as a bound."""


class NextestConfigurationError(TimeoutBudgetError):
    """Raised when the configuration cannot be read at all.

    Separate from a budget that bounds nothing. A file that is not TOML,
    or one declaring no ``slow-timeout`` anywhere, is a configuration
    this contract cannot reason about rather than one whose tiers are in
    the wrong order.
    """


class UnboundedTestError(TimeoutBudgetError):
    """Raised when a ``slow-timeout`` terminates no test.

    ``terminate-after`` is optional, and without it nextest marks a test
    slow and lets it run on, so the configuration parses, reads as
    deliberate, and bounds nothing. Reporting that as a period-long
    budget would put a number on the tier that is missing.
    """
