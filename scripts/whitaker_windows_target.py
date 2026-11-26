"""Keep Whitaker's suite toolchain able to build the Windows target.

Whitaker's lints run inside a compiler driver, so the suite only scores the
``cfg`` surface that compiler was asked to build. The host pass in
``make lint-whitaker`` therefore scores no ``#[cfg(windows)]`` item at all: the
compiler removes those before any lint sees them. ``make
lint-whitaker-windows`` asks for the Windows target instead, which keeps them.

That cross-check needs the Windows target's ``rust-std`` component in the
toolchain the suite runs on. That toolchain is the installer's, not the one
``rust-toolchain.toml`` pins, so the component cannot be added by a bare
``rustup target add``: the channel has to be discovered first. This module owns
that discovery, its ``check`` and ``install`` verbs, and nothing else, so the
Makefile keeps expressing *what* the gate needs rather than how the installer
happens to name its toolchain.

Two sources name a channel, and they are not always the same one:

- ``whitaker-installer list --json`` reports the channels whose suite is
  staged, which is what ``cargo dylint`` can load.
- ``whitaker-installer install --dry-run`` reports the channel the installer
  would build for now, which is what a *successful* install stages.

An upgrade can advance the second without completing the first, and the
bootstrap record the lint run reads keeps naming the suite that survived. So
the candidate set is the union of both: provisioning only the configured
channel would leave the surviving suite without its ``rust-std``, and
provisioning only the staged ones would leave a first install unprovisioned.
Channels whose toolchain ``rustup`` does not have are reported and skipped,
because neither a suite nor a component can run on a compiler that is absent.

Run via ``make install-whitaker-windows-target`` or
``make check-whitaker-windows-target``.
"""

from __future__ import annotations

import argparse
import json
import subprocess  # noqa: S404 - the installer and rustup are the subject.
import typing as typ

#: The line ``whitaker-installer install --dry-run`` prints its toolchain on.
#: The installer writes its report to standard error, so both streams are read.
TOOLCHAIN_PREFIX: typ.Final[str] = "Toolchain: "

#: The default installer executable. The `whitaker` wrapper resolves the same
#: name beside itself or on `PATH`, so a machine that can run the suite can
#: run this.
DEFAULT_INSTALLER: typ.Final[str] = "whitaker-installer"

#: The default target triple. Named here as well as in the Makefile because the
#: module is usable on its own, and the two are held equal by the contract test
#: `tests/workflow_contracts/windows_complexity_gate_test.py`.
DEFAULT_TARGET: typ.Final[str] = "x86_64-pc-windows-msvc"

#: A query's ceiling. Each one inspects local state and none compiles, so a
#: query that outlives this is hung rather than slow.
QUERY_DEADLINE_SECONDS: typ.Final[int] = 300

#: An install's ceiling. It may download the target's standard library.
INSTALL_DEADLINE_SECONDS: typ.Final[int] = 1800


class ToolchainLookupError(RuntimeError):
    """Raised when the suite's toolchain cannot be determined."""


def parse_toolchain(dry_run_output: str) -> str:
    """Return the toolchain channel named in an installer dry-run.

    Parameters
    ----------
    dry_run_output : str
        Everything ``whitaker-installer install --dry-run`` wrote.

    Returns
    -------
    str
        The channel, such as ``nightly-2026-05-28``.

    Raises
    ------
    ToolchainLookupError
        If no ``Toolchain:`` line is present, or it names nothing.

    Examples
    --------
    >>> parse_toolchain("Workspace root: /w\\nToolchain: nightly-2026-05-28\\n")
    'nightly-2026-05-28'
    >>> parse_toolchain("Toolchain:   \\n")
    Traceback (most recent call last):
    ...
    whitaker_windows_target.ToolchainLookupError: the installer named no toolchain
    """
    for line in dry_run_output.splitlines():
        if line.startswith(TOOLCHAIN_PREFIX):
            channel = line[len(TOOLCHAIN_PREFIX) :].strip()
            if channel:
                return channel
            raise ToolchainLookupError("the installer named no toolchain")
    raise ToolchainLookupError(
        f"the installer dry run carried no {TOOLCHAIN_PREFIX.strip()!r} line"
    )


def parse_staged_channels(listing: str) -> list[str]:
    """Return the channels a staged suite is reported for.

    Parameters
    ----------
    listing : str
        The output of ``whitaker-installer list --json``.

    Returns
    -------
    list of str
        The channels, in the order the installer reported them.

    Raises
    ------
    ToolchainLookupError
        If the document is not the expected JSON shape.

    Examples
    --------
    >>> parse_staged_channels('{"toolchains": [{"channel": "nightly-a"}]}')
    ['nightly-a']
    >>> parse_staged_channels('{"toolchains": []}')
    []
    """
    try:
        document = json.loads(listing)
        return [str(entry["channel"]) for entry in document["toolchains"]]
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise ToolchainLookupError(
            f"the installer's suite listing is not the expected JSON: {error}"
        ) from error


def candidate_channels(configured: str, staged: typ.Iterable[str]) -> list[str]:
    """Return every channel a suite could run on, without duplicates.

    Parameters
    ----------
    configured : str
        The channel the installer would build for now.
    staged : iterable of str
        The channels whose suite is already staged.

    Returns
    -------
    list of str
        The union of the two, sorted so a report is order-stable.

    Examples
    --------
    >>> candidate_channels("nightly-b", ["nightly-a", "nightly-b"])
    ['nightly-a', 'nightly-b']
    >>> candidate_channels("nightly-b", [])
    ['nightly-b']
    """
    return sorted({configured, *staged})


def _run(
    argv: list[str],
    *,
    deadline: int,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    """Run one local query or install, capturing both streams.

    Parameters
    ----------
    argv : list of str
        The command line, already split.
    deadline : int
        Seconds the command may take before it is treated as hung.
    check : bool
        Whether a non-zero exit is an error.

    Returns
    -------
    subprocess.CompletedProcess
        The finished process.

    Raises
    ------
    ToolchainLookupError
        If the command cannot be run or outlives its deadline.
    """
    try:
        return subprocess.run(  # noqa: S603 - a fixed argv, no shell.
            argv,
            check=check,
            capture_output=True,
            text=True,
            timeout=deadline,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise ToolchainLookupError(f"{argv[0]} failed: {error}") from error


def configured_channel(installer: str) -> str:
    """Return the channel Whitaker's installer would build its suite for.

    Parameters
    ----------
    installer : str
        The installer executable to query.

    Returns
    -------
    str
        The toolchain channel the next install would stage.
    """
    completed = _run(
        [installer, "install", "--dry-run"], deadline=QUERY_DEADLINE_SECONDS
    )
    return parse_toolchain(completed.stdout + completed.stderr)


def staged_channels(installer: str) -> list[str]:
    """Return the channels whose suite Whitaker has already staged.

    Parameters
    ----------
    installer : str
        The installer executable to query.

    Returns
    -------
    list of str
        The staged channels, empty when nothing is staged yet.
    """
    completed = _run([installer, "list", "--json"], deadline=QUERY_DEADLINE_SECONDS)
    return parse_staged_channels(completed.stdout)


def installed_toolchains() -> list[str]:
    """Return the toolchains ``rustup`` already has, as ``rustup`` names them.

    The names keep their host qualifier, because which hosts exist is
    ``rustup``'s answer and not something to reconstruct from a list here.
    ``installed_targets`` compares a channel against these names instead.

    ``rustup target list --toolchain`` is deliberately not used to decide
    whether a toolchain exists: rustup installs a missing one on demand, so
    that query would download a compiler to answer "is it installed?".

    Returns
    -------
    list of str
        The installed toolchains, host qualifier included.
    """
    completed = _run(["rustup", "toolchain", "list"], deadline=QUERY_DEADLINE_SECONDS)
    return [
        line.split()[0] for line in completed.stdout.splitlines() if line.strip()
    ]


def resolve_toolchains(channel: str, toolchains: typ.Iterable[str]) -> list[str]:
    """Return the installed toolchains *channel* names, by ``rustup``'s names.

    Rustup permits any host, not only the ones this repository builds on, so
    the host is never enumerated here: a name matches when it is the channel
    or the channel plus a host. The host-qualified identities are returned
    rather than the bare channel because they are what the later `rustup`
    queries and installs use. Naming the bare channel would let rustup infer
    the default host, so an installed musl build could stand as proof that the
    unqualified name resolves, and the next query would install a GNU
    compiler to answer. The exact name, when ``rustup`` reports one, comes
    first.

    Parameters
    ----------
    channel : str
        The channel to look for, such as ``nightly-2026-05-28``.
    toolchains : iterable of str
        The names ``rustup toolchain list`` reported.

    Returns
    -------
    list of str
        One identity per installed toolchain naming the channel, empty when
        there is none.

    Examples
    --------
    >>> resolve_toolchains("nightly-a", ["nightly-a-x86_64-unknown-linux-gnu"])
    ['nightly-a-x86_64-unknown-linux-gnu']
    >>> resolve_toolchains("nightly-a", ["nightly-a-x86_64-unknown-linux-musl"])
    ['nightly-a-x86_64-unknown-linux-musl']
    >>> resolve_toolchains("nightly-a", ["nightly-b-x86_64-unknown-linux-gnu"])
    []
    >>> resolve_toolchains("nightly-a", ["nightly-a-x86_64-unknown-linux-gnu", "nightly-a"])
    ['nightly-a', 'nightly-a-x86_64-unknown-linux-gnu']
    """
    named = [
        name for name in toolchains if name == channel or name.startswith(f"{channel}-")
    ]
    return [name for name in named if name == channel] + [
        name for name in named if name != channel
    ]


def installed_targets(toolchain: str) -> set[str]:
    """Return the triples ``rustup`` has for one installed *toolchain*.

    Parameters
    ----------
    toolchain : str
        A name ``rustup toolchain list`` reported, host qualifier included.

    Returns
    -------
    set of str
        The installed triples, empty when the toolchain has none.
    """
    completed = _run(
        ["rustup", "target", "list", "--installed", "--toolchain", toolchain],
        deadline=QUERY_DEADLINE_SECONDS,
    )
    return set(completed.stdout.split())


def install_target(toolchain: str, target: str) -> None:
    """Add *target*'s standard library to one installed *toolchain*.

    The argument is an identity ``rustup`` reported, not a bare channel, for
    the reason `resolve_toolchains` returns those: a bare channel lets rustup
    infer the default host, which may be a compiler that is not installed.

    Parameters
    ----------
    toolchain : str
        The toolchain identity to extend.
    target : str
        The target triple to add.
    """
    _run(
        ["rustup", "target", "add", "--toolchain", toolchain, target],
        deadline=INSTALL_DEADLINE_SECONDS,
    )


def survey(
    installer: str, target: str
) -> tuple[list[str], list[str], list[str]]:
    """Report which installed toolchains have, need, or cannot take the target.

    Parameters
    ----------
    installer : str
        The installer executable to query.
    target : str
        The target triple the gate needs.

    Returns
    -------
    tuple of list of str
        The toolchain identities that already have the target, those that do
        not, and the candidate channels ``rustup`` does not have at all.

    Raises
    ------
    ToolchainLookupError
        If no candidate channel has an installed toolchain, because then
        there is nothing to provision and nothing that could run the suite.
    """
    channels = candidate_channels(
        configured_channel(installer), staged_channels(installer)
    )
    toolchains = installed_toolchains()
    satisfied: list[str] = []
    missing: list[str] = []
    absent: list[str] = []
    for channel in channels:
        identities = resolve_toolchains(channel, toolchains)
        if not identities:
            absent.append(channel)
            continue
        for identity in identities:
            if target in installed_targets(identity):
                satisfied.append(identity)
            else:
                missing.append(identity)
    if not satisfied and not missing:
        raise ToolchainLookupError(
            "no candidate Whitaker toolchain is installed: "
            + ", ".join(absent or ["none reported"])
        )
    return satisfied, missing, absent


def build_parser() -> argparse.ArgumentParser:
    """Return the command-line parser for this module.

    Returns
    -------
    argparse.ArgumentParser
        A parser with the ``check`` and ``install`` verbs and the shared
        ``--target`` and ``--installer`` options.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "verb",
        choices=("check", "install"),
        help="check that the target is installed, or install it",
    )
    parser.add_argument("--target", default=DEFAULT_TARGET)
    parser.add_argument("--installer", default=DEFAULT_INSTALLER)
    return parser


def main(argv: typ.Sequence[str] | None = None) -> int:
    """Run one verb and report the outcome in the gate's own words.

    Parameters
    ----------
    argv : sequence of str or None
        Arguments to parse; ``None`` reads ``sys.argv``.

    Returns
    -------
    int
        ``0`` on success, ``1`` when the target is missing (``check`` only) or
        the suite's toolchain cannot be determined.
    """
    arguments = build_parser().parse_args(argv)
    try:
        satisfied, missing, absent = survey(arguments.installer, arguments.target)
    except ToolchainLookupError as error:
        print(f"whitaker windows target: {error}")
        return 1
    for channel in absent:
        print(
            f"whitaker windows target: {channel} is staged but its toolchain is "
            "not installed; skipping it",
        )
    if arguments.verb == "install":
        for channel in missing:
            install_target(channel, arguments.target)
            print(
                f"whitaker windows target: {arguments.target} installed for {channel}"
            )
        if not missing:
            print(
                f"whitaker windows target: {arguments.target} is installed for "
                + ", ".join(satisfied)
            )
        return 0
    for channel in satisfied:
        print(
            f"whitaker windows target: {arguments.target} is installed for {channel}"
        )
    if missing:
        print(
            f"whitaker windows target: {arguments.target} is missing from "
            + ", ".join(missing)
            + ".\nRun `make install-whitaker-windows-target` to add it.",
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())