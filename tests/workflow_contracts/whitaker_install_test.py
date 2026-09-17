"""Contract tests for the Whitaker install step's two fail-closed properties.

On 2026-09-17 this repository's `build-test (ubuntu-latest)` job failed at
"Install Whitaker", and the failure was neither the installer version nor an
unlocked `cargo install`. Both the failing attempt and its successful re-run
resolved whitaker-installer 0.2.8.

What happened, from the attempt's own log: `cargo binstall` asked the GitHub
API for the trailofbits/dylint release, received `403 Forbidden` twice, saw its
other fetchers time out, and then reported `The package dylint-link v6.0.1 will
be installed from source (with cargo)`. That source build resolves
`cargo-platform` 0.3.3, which declares rustc 1.91 against this repository's
1.89 pin, so it cannot complete. The step's own preinstall of the prebuilt
dylint-link exists precisely to avoid that build, and binstall's fallback
defeated it.

Two properties follow, and neither is visible in a passing run, which is why
they are asserted here rather than left to the next cold cache.

The step must be authenticated. Anonymous GitHub API requests share the
runner's address-based rate limit with every other job on the fleet, so the
403 is a matter of neighbours rather than of this repository. The resolve step
above already uses `github.token` for the same API.

And binstall must not be allowed to compile. With `--disable-strategies
compile` an unreachable release is an error at the download, naming the
release; without it the job spends minutes building and then fails on a rustc
version, which reads as a toolchain problem and sends the reader to the wrong
place entirely.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

import pytest
import yaml

WORKFLOW_PATH: typ.Final[Path] = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"
)

#: The step this contract is about.
STEP_NAME: typ.Final[str] = "Install Whitaker"

#: The expression that supplies the workflow's own token. Asserted by
#: value rather than by presence: a token variable set to something
#: else, or to an empty string, authenticates nothing while reading
#: exactly like this does in a diff.
TOKEN_EXPRESSION: typ.Final[str] = "${{ github.token }}"

#: Every name binstall and the GitHub CLI read a token from. Both are
#: set because the step runs both kinds of tool, and a reader should not
#: have to know which of them consults which.
TOKEN_VARIABLES: typ.Final[frozenset[str]] = frozenset({"GH_TOKEN", "GITHUB_TOKEN"})

#: The flag that makes an unreachable release an error rather than a
#: source build.
NO_COMPILE: typ.Final[str] = "--disable-strategies compile"

#: A `cargo binstall` invocation that installs something. The `-V`
#: probe is deliberately not matched: it installs nothing, so the flag
#: would say nothing about it, and requiring it there would be cargo
#: cult rather than contract.
#:
#: Matched after the shell's line continuations are joined, because an
#: invocation wrapped across lines is the same command and the flag may
#: sit on either side of the break. The first version of this pattern
#: tried to match the continuation itself and silently found only the
#: first line of each; the count assertion below is what caught it.
_BINSTALL_INSTALL: typ.Final[re.Pattern[str]] = re.compile(
    r"cargo binstall\s+--no-confirm\b[^\n]*"
)


def _joined(script: str) -> str:
    """Return a shell script with its line continuations joined.

    Parameters
    ----------
    script : str
        The step's `run` body.

    Returns
    -------
    str
        The same script with each ``\\``-newline pair replaced by a
        space, so one command occupies one line.
    """
    return re.sub(r"\\\n\s*", " ", script)


def _install_step() -> dict[str, typ.Any]:
    """Return the Install Whitaker step's mapping.

    Returns
    -------
    dict
        The step as the workflow declares it.

    Raises
    ------
    AssertionError
        If the workflow does not parse, or declares no such step. Both
        are asserted rather than tolerated: every assertion below is
        satisfied by a reading that finds nothing.
    """
    document = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    assert isinstance(document, dict), f"{WORKFLOW_PATH.name} must parse as a mapping"
    jobs = document.get("jobs")
    assert isinstance(jobs, dict), f"{WORKFLOW_PATH.name} must declare jobs"
    matches = [
        step
        for job in jobs.values()
        if isinstance(job, dict)
        for step in (job.get("steps") or [])
        if isinstance(step, dict) and step.get("name") == STEP_NAME
    ]
    assert len(matches) == 1, (
        f"expected exactly one {STEP_NAME!r} step in {WORKFLOW_PATH.name}, "
        f"found {len(matches)}"
    )
    return matches[0]


@pytest.mark.parametrize("variable", sorted(TOKEN_VARIABLES))
def test_the_install_step_is_authenticated(variable: str) -> None:
    """Assert the step passes the workflow's token to the tools it runs.

    Anonymous GitHub API requests are rate limited by address, so this
    step competes with every other job on the runner fleet. The 403 that
    broke it was a neighbour's traffic, not this repository's, which is
    why the remedy is a token rather than a retry.

    Asserted by value. A token variable set to an empty string, or to
    some other expression, authenticates nothing and looks identical in
    a diff to one that works.
    """
    environment = _install_step().get("env") or {}
    assert environment.get(variable) == TOKEN_EXPRESSION, (
        f"{STEP_NAME} must set {variable} to {TOKEN_EXPRESSION} so its GitHub "
        f"API requests are authenticated and not subject to the anonymous "
        f"rate limit shared across the runner fleet; it sets "
        f"{environment.get(variable)!r}"
    )


def test_every_binstall_install_refuses_to_compile() -> None:
    """Assert no binstall invocation may fall back to building from source.

    This is the property that actually failed. binstall's fallback is
    fail-open: an unreachable release becomes a source build, and the
    source build needs a newer rustc than this repository pins, so the
    job dies several minutes later on a toolchain message that sends the
    reader nowhere near the download that failed.

    Every installing invocation is checked rather than the one that
    broke, because the step installs two things and either can be the
    one whose release is unreachable next time.
    """
    script = _joined(str(_install_step().get("run", "")))
    invocations = _BINSTALL_INSTALL.findall(script)
    assert invocations, (
        f"{STEP_NAME} runs no `cargo binstall --no-confirm` invocation, so "
        f"this contract is reading the wrong step or the wrong workflow"
    )
    uncapped = [
        invocation.strip()
        for invocation in invocations
        if NO_COMPILE not in invocation
    ]
    assert not uncapped, (
        f"these {STEP_NAME} invocations may fall back to a source build, "
        f"which this repository's pinned toolchain cannot complete; each "
        f"needs {NO_COMPILE!r}: {uncapped}"
    )


def test_the_sweep_finds_both_invocations() -> None:
    """Assert the reading finds the two installs the step actually makes.

    Both assertions above are satisfied by a pattern that matches
    nothing, and the emptiness check only rules out finding none. The
    step installs whitaker-installer and dylint-link, so the count is
    pinned: a pattern that quietly stopped matching the continuation
    lines would otherwise report one and pass.
    """
    script = _joined(str(_install_step().get("run", "")))
    invocations = _BINSTALL_INSTALL.findall(script)
    assert len(invocations) == 2, (
        f"{STEP_NAME} installs whitaker-installer and dylint-link, so the "
        f"sweep must find two invocations; it found {len(invocations)}: "
        f"{invocations}"
    )
    joined = "\n".join(invocations)
    for package in ("whitaker-installer", "dylint-link"):
        assert package in joined, (
            f"the sweep missed the {package} invocation, so the assertion "
            f"above passed over it"
        )
