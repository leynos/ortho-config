#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts>=2.9"]
# ///
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

import json
import os
import pathlib
import shlex
import shutil
import subprocess  # noqa: S404 - the installer and rustup are the subject.
import typing as typ

import cyclopts

#: The line ``whitaker-installer install --dry-run`` prints its toolchain on.
#: The installer writes its report to standard error, so both streams are read.
TOOLCHAIN_PREFIX: typ.Final[str] = "Toolchain: "

#: The default launcher. Its directory is where the wrapper looks first for the
#: installer, so it decides which installation this helper surveys.
DEFAULT_LAUNCHER: typ.Final[str] = "whitaker"

#: The default installer executable, used when none is staged beside the
#: launcher. It is on `PATH` on a machine where the wrapper works.
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


def unquote(word: str) -> str:
    """Remove one pair of enclosing quotes from a word.

    Non-POSIX splitting keeps the quotes that delimit a Windows path, and
    `shutil.which` cannot resolve a path that still carries them. Backslashes
    are untouched, which is the reason the non-POSIX mode is used at all.

    Parameters
    ----------
    word : str
        One word, possibly quoted.

    Returns
    -------
    str
        The word without its enclosing quotes.

    Examples
    --------
    >>> unquote('"C:\\\\Program Files\\\\Whitaker\\\\whitaker.exe"')
    'C:\\\\Program Files\\\\Whitaker\\\\whitaker.exe'
    >>> unquote("whitaker")
    'whitaker'
    """
    for quote in ('"', "'"):
        if len(word) >= 2 and word.startswith(quote) and word.endswith(quote):
            return word[1:-1]
    return word


def split_launcher(launcher: str) -> list[str]:
    """Return the argv for a launcher command, without running a shell.

    `WHITAKER` may name one executable or a command with arguments, such as
    `env whitaker`, so the first word is what names the directory to search.
    Splitting is POSIX on POSIX and keeps backslashes on Windows, where the
    enclosing quotes are then removed by `unquote`.

    Parameters
    ----------
    launcher : str
        The launcher as the caller configured it.

    Returns
    -------
    list of str
        The command and its leading arguments.

    Examples
    --------
    >>> split_launcher("whitaker")
    ['whitaker']
    >>> split_launcher("env whitaker")
    ['env', 'whitaker']
    """
    return [unquote(word) for word in shlex.split(launcher, posix=os.name != "nt")]


def launcher_program(launcher: str) -> pathlib.Path | None:
    """Return the executable a launcher command runs, as an absolute path.

    A launcher may carry arguments: `env /opt/suite/bin/whitaker` wraps the
    executable this helper has to search beside, and resolving `env` instead
    would look in the wrong directory. So the program is the last word that
    `shutil.which` resolves, which handles a wrapper, a bare name, a relative
    path and a program followed by options. An argument that happens to name
    an executable is the price of not parsing the command, and the first word
    is the fallback when no word resolves.

    Parameters
    ----------
    launcher : str
        The launcher as the caller configured it.

    Returns
    -------
    pathlib.Path or None
        The absolute path of the program, or ``None`` when nothing resolves.
    """
    for word in reversed(split_launcher(launcher)):
        resolved = shutil.which(word)
        if resolved:
            # Absolute, but not resolved through symlinks: the wrapper finds
            # its installer beside the path it was invoked as, which is where
            # `BASH_SOURCE[0]` points, not where a symlink leads.
            return pathlib.Path(os.path.abspath(resolved))
    return None


def installer_beside(launcher: str) -> pathlib.Path | None:
    """Return the installer staged beside *launcher*, when one is there.

    The `whitaker` wrapper resolves `whitaker-installer` from its own directory
    first and from `PATH` second, so searching the same way is what keeps this
    helper surveying the installation the launcher actually runs. Naming the
    `PATH` installer unconditionally would check, and provision, a different
    installation from the one the gate lints with.

    The path is absolute because the sibling has to survive being handed to a
    child process: a launcher given as `./whitaker` would otherwise yield a
    bare `whitaker-installer`, which that child resolves against its own
    `PATH` rather than against the directory the launcher lives in.

    Parameters
    ----------
    launcher : str
        The launcher as the caller configured it.

    Returns
    -------
    pathlib.Path or None
        The installer beside the launcher, or ``None`` when there is none.
    """
    program = launcher_program(launcher)
    if program is None:
        return None
    suffix = ".exe" if os.name == "nt" else ""
    candidate = program.with_name(f"{DEFAULT_INSTALLER}{suffix}")
    if candidate.is_file() and os.access(candidate, os.X_OK):
        return candidate
    return None


def default_installer(launcher: str) -> str:
    """Return the installer to query for *launcher*.

    Parameters
    ----------
    launcher : str
        The launcher as the caller configured it.

    Returns
    -------
    str
        The installer beside the launcher when there is one, and the bare name
        otherwise, which `PATH` then resolves.
    """
    beside = installer_beside(launcher)
    return str(beside) if beside else DEFAULT_INSTALLER


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


def candidate_channels(
    configured: str | None, staged: typ.Iterable[str]
) -> list[str]:
    """Return every channel a suite could run on, without duplicates.

    Parameters
    ----------
    configured : str or None
        The channel the installer would build for now, or ``None`` when it
        could not report one.
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
    >>> candidate_channels(None, ["nightly-a"])
    ['nightly-a']
    """
    channels = {*staged}
    if configured is not None:
        channels.add(configured)
    return sorted(channels)


def _run(
    argv: list[str],
    *,
    deadline: int,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    """Run one local query or install, capturing both streams.

    This is `subprocess` rather than `cuprum`, which the scripting standards
    otherwise require, because every call here is bounded by a deadline and
    `cuprum` 0.1.0 — the version the standards pin — has none. The deadlines
    are what stop a stalled `rustup` or installer from occupying a gate
    indefinitely. `cuprum` 0.2.0b2 adds a `timeout`, and this should move to
    it once that release is no longer a beta.

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


def configured_channel(installer: str) -> str | None:
    """Return the channel Whitaker's installer would build its suite for.

    A dry run verifies that its configured toolchain is installed before it
    prints anything, so an upgrade that advanced the configuration without
    staging the new suite makes it fail. That is precisely the state the
    staged channels exist to survive, so a failed dry run yields ``None``
    rather than aborting the survey. A launch failure — no installer at all —
    still raises, because then nothing can be discovered.

    Parameters
    ----------
    installer : str
        The installer executable to query.

    Returns
    -------
    str or None
        The channel the next install would stage, or ``None`` when the
        installer cannot report one.
    """
    completed = _run(
        [installer, "install", "--dry-run"],
        deadline=QUERY_DEADLINE_SECONDS,
        check=False,
    )
    if completed.returncode != 0:
        return None
    try:
        return parse_toolchain(completed.stdout + completed.stderr)
    except ToolchainLookupError:
        return None


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
    # Staged channels are read first, so a configured channel the installer
    # cannot verify never prevents the surviving suites from being surveyed.
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


def _survey_or_report(
    installer: str, target: str
) -> tuple[list[str], list[str], list[str]] | None:
    """Survey the toolchains, reporting a lookup failure in the gate's words.

    Parameters
    ----------
    installer : str
        The installer executable to query.
    target : str
        The target triple the gate needs.

    Returns
    -------
    tuple of list of str or None
        The survey, or ``None`` when it could not be made and the failure has
        already been printed.
    """
    try:
        return survey(installer, target)
    except ToolchainLookupError as error:
        print(f"whitaker windows target: {error}")
        return None


def _report_absent(absent: typ.Sequence[str]) -> None:
    """Say which staged channels have no installed toolchain, and skip them.

    Parameters
    ----------
    absent : sequence of str
        The candidate channels ``rustup`` does not have.
    """
    for channel in absent:
        print(
            f"whitaker windows target: {channel} is staged but its toolchain is "
            "not installed; skipping it",
        )


def check(*, target: str = DEFAULT_TARGET, launcher: str = DEFAULT_LAUNCHER,
          installer: str | None = None) -> int:
    """Report whether the Windows target is installed for the suite's toolchain.

    Parameters
    ----------
    target : str
        The target triple the gate needs.
    launcher : str
        The Whitaker launcher whose installer decides which suite is surveyed.
    installer : str or None
        An explicit installer; ``None`` uses the one staged beside the launcher.

    Returns
    -------
    int
        ``0`` when every installed candidate has the target, ``1`` otherwise.
    """
    survey_result = _survey_or_report(installer or default_installer(launcher), target)
    if survey_result is None:
        return 1
    satisfied, missing, absent = survey_result
    _report_absent(absent)
    for channel in satisfied:
        print(f"whitaker windows target: {target} is installed for {channel}")
    if missing:
        print(
            f"whitaker windows target: {target} is missing from "
            + ", ".join(missing)
            + ".\nRun `make install-whitaker-windows-target` to add it.",
        )
        return 1
    return 0


def install(*, target: str = DEFAULT_TARGET, launcher: str = DEFAULT_LAUNCHER,
            installer: str | None = None) -> int:
    """Install the Windows target for every suite toolchain that lacks it.

    A failure stops the run but keeps whatever was already installed, so a
    retry surveys the completed ones rather than starting over.

    Parameters
    ----------
    target : str
        The target triple to provision.
    launcher : str
        The Whitaker launcher whose installer decides which suite is surveyed.
    installer : str or None
        An explicit installer; ``None`` uses the one staged beside the launcher.

    Returns
    -------
    int
        ``0`` when every candidate has the target, ``1`` when one could not be
        installed or the suite's toolchain could not be determined.
    """
    survey_result = _survey_or_report(installer or default_installer(launcher), target)
    if survey_result is None:
        return 1
    satisfied, missing, absent = survey_result
    _report_absent(absent)
    try:
        for channel in missing:
            install_target(channel, target)
            print(f"whitaker windows target: {target} installed for {channel}")
    except ToolchainLookupError as error:
        print(f"whitaker windows target: {error}")
        return 1
    if not missing:
        print(
            f"whitaker windows target: {target} is installed for "
            + ", ".join(satisfied)
        )
    return 0


app = cyclopts.App(
    name="whitaker-windows-target",
    help=__doc__.splitlines()[0],
)
app.command(check)
app.command(install)



if __name__ == "__main__":
    app()