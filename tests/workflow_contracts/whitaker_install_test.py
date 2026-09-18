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

And binstall must not be allowed to compile `dylint-link`. With
`--disable-strategies compile` an unreachable release is an error at the
download, naming the release; without it the job spends minutes building and
then fails on a rustc version, which reads as a toolchain problem and sends the
reader to the wrong place entirely.

The `whitaker-installer` install is exempt, and the exemption is asserted
rather than left to a reader's judgement. binstall cannot install that crate
from its release at all: on 2026-09-18 it resolved the asset, downloaded it,
and reported `bin whitaker-installer is not found` for a path the tarball does
contain. Its `--locked` source build is therefore the only working path, and
unlike the dylint-link one it completes, because the lockfile keeps
`cargo-platform` off the version that requires rustc 1.91.

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


def _invocation_installing(package: str) -> str:
    """Return the single binstall invocation that installs one package.

    Parameters
    ----------
    package : str
        The crate name the invocation must name.

    Returns
    -------
    str
        The invocation, with its line continuations joined.

    Raises
    ------
    AssertionError
        If the step makes no such invocation, or more than one.
    """
    script = _joined(str(_install_step().get("run", "")))
    matches = [
        invocation.strip()
        for invocation in _BINSTALL_INSTALL.findall(script)
        if package in invocation
    ]
    assert len(matches) == 1, (
        f"expected exactly one `cargo binstall` invocation installing "
        f"{package} in {STEP_NAME}; found {len(matches)}: {matches}"
    )
    return matches[0]


def test_the_dylint_link_install_refuses_to_compile() -> None:
    """Assert the install that cannot be compiled is not allowed to try.

    This is the property that actually failed. `dylint-link@6.0.1` is
    installed without `--locked`, so a source build resolves
    `cargo-platform` 0.3.3, which declares rustc 1.91 against this
    repository's 1.89 pin and cannot complete. binstall's fallback is
    fail-open: an unreachable release quietly becomes that build, and
    the job dies several minutes later on a toolchain message that
    sends the reader nowhere near the download that failed.

    Asserted on this invocation by name rather than on every one, and
    the neighbouring test says why the other is exempt.
    """
    invocation = _invocation_installing("dylint-link")
    assert NO_COMPILE in invocation, (
        f"the dylint-link install may fall back to a source build, which "
        f"this repository's pinned toolchain cannot complete; it needs "
        f"{NO_COMPILE!r}: {invocation!r}"
    )


def test_the_installer_install_keeps_its_locked_source_fallback() -> None:
    """Assert the exemption above is narrow, and pin why it exists.

    binstall cannot install `whitaker-installer` from its GitHub release
    today. On 2026-09-18 it resolved the asset, downloaded it, and then
    reported `bin whitaker-installer is not found`, although the tarball
    contains `whitaker-installer-x86_64-unknown-linux-gnu-v0.2.8/whitaker-installer`,
    which is the exact path the message names.

    So the `--locked` source build is the only path that works for this
    crate, and it does work: the lockfile keeps `cargo-platform` off
    0.3.3, so the rustc 1.91 requirement that breaks the dylint-link
    build never arises here.

    Refusing the fallback on this invocation therefore fails a working
    lane rather than protecting it, which is what happened when this
    contract first asserted the flag on both. The assertion is inverted
    deliberately: adding the flag here must fail, so nobody restores it
    from the symmetry of the two lines alone. It comes back when the
    binstall failure is understood, and this test is where the reason is
    recorded.
    """
    invocation = _invocation_installing("whitaker-installer")
    assert NO_COMPILE not in invocation, (
        f"the whitaker-installer install must keep its --locked source "
        f"fallback: binstall cannot install this crate from its release, "
        f"so {NO_COMPILE!r} fails the job on the only path that works; "
        f"{invocation!r}"
    )
    assert "--locked" in invocation, (
        f"the whitaker-installer source fallback must be --locked, which is "
        f"what keeps cargo-platform off the version that needs rustc 1.91; "
        f"{invocation!r}"
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
