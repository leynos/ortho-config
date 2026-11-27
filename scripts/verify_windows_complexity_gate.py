#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Prove the Windows-target complexity gate passes and fails as specified.

``make lint-whitaker-windows`` is only evidence if it can still report
something. The suite is a rolling release, so a revision can change what the
lint scores, and the command could also lose the flag that keeps Windows-only
items in the compilation without any commit here failing.

This check therefore runs the same command over
``tests/fixtures/windows_complexity_probe``, a crate whose every item is
``#[cfg(windows)]``:

- the default build carries only an in-limit example and must exit zero;
- the build with the crate's ``over_limit`` feature adds an example that
  scores one above the configured limit, and must exit non-zero naming that
  function.

The limit is read from the repository's ``dylint.toml`` rather than repeated
here, so the probe is scored at the same threshold the workspace gate uses.

Run via ``make check-windows-complexity-gate``.
"""

from __future__ import annotations

import os
import pathlib
import re
import shlex
import signal
import subprocess  # noqa: S404 - the gate is an external command, the subject.
import time
import typing as typ

import cyclopts

#: The repository root, from this file's location under `scripts/`.
REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[1]

#: The probe crate, relative to the repository root.
PROBE_DIRECTORY: typ.Final[pathlib.Path] = (
    REPOSITORY_ROOT / "tests" / "fixtures" / "windows_complexity_probe"
)

#: The file the threshold is configured in.
DYLINT_CONFIGURATION: typ.Final[pathlib.Path] = REPOSITORY_ROOT / "dylint.toml"

#: The function the over-limit build must be reported for.
OVER_LIMIT_FUNCTION: typ.Final[str] = "over_limit"

#: The function that must never be reported.
IN_LIMIT_FUNCTION: typ.Final[str] = "in_limit"

#: The warning policy the Makefile's `STRICT_RUSTFLAGS` states, which is what
#: turns the lint's `warn` default into a gate.
STRICT_RUSTFLAGS: typ.Final[str] = "-D warnings"

#: One probe invocation's ceiling. The crate has no dependencies of its own
#: beyond the standard library, so the only slow part is the first build of
#: the target's `std`, which the job's own ceiling already accommodates.
INVOCATION_DEADLINE_SECONDS: typ.Final[int] = 1800

#: How long a terminated probe's process group has to exit before it is
#: killed. A compiler that ignores the first signal still releases its locks
#: when it dies.
TERMINATION_GRACE_SECONDS: typ.Final[int] = 30

#: How often the group is tested while the grace period runs. Short enough
#: that a cooperative launcher is noticed promptly, long enough to cost
#: nothing.
GROUP_POLL_SECONDS: typ.Final[float] = 0.5

#: A diagnostic such as `has a cognitive complexity of 12 (limit 9)`.
COMPLEXITY_DIAGNOSTIC: typ.Final[re.Pattern[str]] = re.compile(
    r"has a cognitive complexity of (\d+) \(limit (\d+)\)"
)

#: The `max_complexity` assignment in `dylint.toml`.
THRESHOLD_SETTING: typ.Final[re.Pattern[str]] = re.compile(
    r"^\s*max_complexity\s*=\s*(\d+)\s*$", re.MULTILINE
)


class GateVerificationError(RuntimeError):
    """Raised when the probe shows the gate behaving other than specified."""


def configured_limit(configuration: str) -> int:
    """Return the `max_complexity` the configuration sets.

    Parameters
    ----------
    configuration : str
        The text of a `dylint.toml`.

    Returns
    -------
    int
        The configured limit.

    Raises
    ------
    GateVerificationError
        If the file sets no limit, or sets more than one.

    Examples
    --------
    >>> configured_limit("[macro_aware_cognitive_complexity]\\nmax_complexity = 9\\n")
    9
    >>> configured_limit("[whitaker]\\nexperimental = []\\n")
    Traceback (most recent call last):
    ...
    verify_windows_complexity_gate.GateVerificationError: no max_complexity setting
    """
    limits = THRESHOLD_SETTING.findall(configuration)
    if not limits:
        raise GateVerificationError("no max_complexity setting")
    if len(set(limits)) > 1:
        raise GateVerificationError(f"several max_complexity settings: {limits}")
    return int(limits[0])


def unquote(word: str) -> str:
    """Remove one pair of enclosing quotes from a word.

    Non-POSIX splitting keeps the quotes that delimit a Windows path, and
    `subprocess` would look for a file whose name contains them. Backslashes
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

    The Makefile passes `WHITAKER` through `shell_quote`, so the value may be
    one executable or a command with arguments, such as `env whitaker`. It is
    split here rather than handed to `subprocess` whole, which would look for
    a file whose name contains a space. No shell is involved either way.

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


def gate_command(*, whitaker: str, target: str, over_limit: bool) -> list[str]:
    """Return the `whitaker` arguments that score the probe crate.

    Parameters
    ----------
    whitaker : str
        The Whitaker launcher to run.
    target : str
        The target triple to check at.
    over_limit : bool
        Whether to enable the probe's over-limit example.

    Returns
    -------
    list of str
        The complete command line.

    Examples
    --------
    >>> gate_command(whitaker="whitaker", target="x86_64-pc-windows-msvc",
    ...              over_limit=False)[:2]
    ['whitaker', '--all']
    >>> gate_command(whitaker="env whitaker", target="t", over_limit=False)[:3]
    ['env', 'whitaker', '--all']
    >>> "--features" in gate_command(whitaker="w", target="t", over_limit=True)
    True
    """
    command = [
        *split_launcher(whitaker),
        "--all",
        "--manifest-path",
        str(PROBE_DIRECTORY / "Cargo.toml"),
        "--",
        "--all-targets",
        "--target",
        target,
    ]
    if over_limit:
        command += ["--features", "over_limit"]
    return command


def run_gate(
    *, whitaker: str, target: str, over_limit: bool
) -> subprocess.CompletedProcess[str]:
    """Run the gate over the probe crate and return the finished process.

    The deadline is per invocation, and each one compiles a single crate: a
    cold start pays for the target's dependency graph once, and the second
    invocation reuses it. A hung compiler therefore fails here rather than
    occupying a runner until the job's own ceiling.

    This is `subprocess` rather than `cuprum`, which the scripting standards
    otherwise require, because the two properties this probe needs are the
    two `cuprum` does not offer at the version the standards pin (0.1.0): a
    wall-clock deadline, and a process group that can be signalled as a unit
    so a launcher's compilers cannot outlive it and hold their build locks.
    `cuprum` 0.2.0b2 adds both, and this should move to it once that release
    is no longer a beta.

    Parameters
    ----------
    whitaker : str
        The Whitaker launcher to run.
    target : str
        The target triple to check at.
    over_limit : bool
        Whether to enable the probe's over-limit example.

    Returns
    -------
    subprocess.CompletedProcess
        The finished process, with both streams captured.

    Raises
    ------
    GateVerificationError
        If the invocation outlives its deadline or cannot be started.
    """
    argv = gate_command(whitaker=whitaker, target=target, over_limit=over_limit)
    try:
        process = subprocess.Popen(  # noqa: S603 - a fixed argv, no shell.
            argv,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=gate_environment(),
            start_new_session=True,
        )
    except OSError as error:
        raise GateVerificationError(f"could not run {whitaker!r}: {error}") from error
    # `start_new_session` makes the launcher the leader of its own group, so
    # its pid is the group id. Keeping it means the group can still be
    # signalled after the launcher itself has exited.
    group = None if os.name == "nt" else process.pid
    try:
        stdout, stderr = process.communicate(timeout=INVOCATION_DEADLINE_SECONDS)
    except subprocess.TimeoutExpired as expired:
        terminate_group(process, group)
        drain(process)
        raise GateVerificationError(
            f"the {target} probe did not finish within "
            f"{INVOCATION_DEADLINE_SECONDS}s"
        ) from expired
    except KeyboardInterrupt:
        # Ctrl+C interrupts this process, not the session the probe leads.
        terminate_group(process, group)
        drain(process)
        raise
    return subprocess.CompletedProcess(argv, process.returncode, stdout, stderr)


def _signal_group(group: int, number: int) -> None:
    """Send *number* to *group*, ignoring a group that has already gone.

    Parameters
    ----------
    group : int
        The process group to signal.
    number : int
        The signal number to send.
    """
    try:
        os.killpg(group, number)
    except ProcessLookupError:
        return


def _group_alive(group: int) -> bool:
    """Report whether any member of *group* is still running.

    Signal zero tests for existence without changing anything, and it covers
    the case `process.wait` cannot: the launcher may have exited while a
    descendant it started still runs.

    Parameters
    ----------
    group : int
        The process group to test.

    Returns
    -------
    bool
        Whether the group still has a member.
    """
    try:
        os.killpg(group, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def terminate_group(process: subprocess.Popen[str], group: int | None) -> None:
    """Stop one probe process and every descendant it started.

    `subprocess` signals only the process it spawned. A launcher such as
    `cargo dylint` starts compilers of its own, and those would keep their
    build locks after this script reported failure, which on a shared host is
    worse than the failure. The probe is therefore started in a session of its
    own, and the whole session is signalled here: politely first, then
    forcibly once the grace period has passed with members still alive. The
    grace period is measured against the group rather than the launcher,
    because the launcher exiting does not mean its descendants have. On
    Windows the session concept does not exist, and the launcher is simply
    killed.

    Parameters
    ----------
    process : subprocess.Popen
        The probe to stop.
    group : int or None
        The process group to signal, or ``None`` on Windows.
    """
    if group is None:
        process.kill()
        return
    _signal_group(group, signal.SIGTERM)
    deadline = time.monotonic() + TERMINATION_GRACE_SECONDS
    while time.monotonic() < deadline:
        if not _group_alive(group):
            return
        time.sleep(GROUP_POLL_SECONDS)
    _signal_group(group, signal.SIGKILL)


def drain(process: subprocess.Popen[str]) -> None:
    """Reap *process* and read what its pipes still hold, for a bounded time.

    A descendant that outlived the group may still hold the pipe open, and an
    unbounded read would wait for it indefinitely. The read is therefore
    bounded, and a pipe that stays open is abandoned rather than blocking the
    failure report.

    Parameters
    ----------
    process : subprocess.Popen
        The probe whose pipes are drained.
    """
    for attempt in range(2):
        try:
            process.communicate(timeout=TERMINATION_GRACE_SECONDS)
        except subprocess.TimeoutExpired:
            if attempt == 0:
                process.kill()
            continue
        return


def gate_environment() -> dict[str, str]:
    """Return the environment that gives the gate the Makefile's warning policy.

    The caller's environment is kept: the launcher resolves the installer, the
    toolchain and the package cache through it, and dropping any of them would
    make the probe fail for a reason that is not the gate's.

    `CARGO_ENCODED_RUSTFLAGS` is removed rather than left alone. Cargo reads it
    in preference to `RUSTFLAGS`, and an empty value wins as well, so an
    inherited one would displace the warning policy this probe exists to apply
    and the over-limit example would merely warn. The Makefile un-exports the
    same variable for the same reason.

    Returns
    -------
    dict of str to str
        The inherited environment with `RUSTFLAGS` replaced and
        `CARGO_ENCODED_RUSTFLAGS` removed.
    """
    environment = dict(os.environ)
    environment["RUSTFLAGS"] = STRICT_RUSTFLAGS
    environment.pop("CARGO_ENCODED_RUSTFLAGS", None)
    return environment


def verify(*, whitaker: str, target: str) -> list[str]:
    """Check both directions and return the lines to report.

    Parameters
    ----------
    whitaker : str
        The Whitaker launcher to run.
    target : str
        The target triple to check at.

    Returns
    -------
    list of str
        One line per demonstrated direction, with the evidence inline.

    Raises
    ------
    GateVerificationError
        If either direction does not behave as specified.
    """
    limit = configured_limit(DYLINT_CONFIGURATION.read_text(encoding="utf-8"))
    accepted = run_gate(whitaker=whitaker, target=target, over_limit=False)
    if accepted.returncode != 0:
        raise GateVerificationError(
            "the in-limit example did not pass:\n" + accepted.stdout + accepted.stderr
        )
    reported = run_gate(whitaker=whitaker, target=target, over_limit=True)
    output = reported.stdout + reported.stderr
    if reported.returncode == 0:
        raise GateVerificationError(
            f"the over-limit example passed at a limit of {limit}:\n{output}"
        )
    diagnostics = COMPLEXITY_DIAGNOSTIC.findall(output)
    if not diagnostics:
        raise GateVerificationError(
            "the over-limit build failed without a complexity diagnostic:\n" + output
        )
    scored, stated_limit = (int(value) for value in diagnostics[0])
    if scored <= stated_limit:
        raise GateVerificationError(
            f"the reported score {scored} is within the stated limit {stated_limit}"
        )
    if stated_limit != limit:
        raise GateVerificationError(
            f"the gate stated a limit of {stated_limit}, but dylint.toml sets {limit}"
        )
    if OVER_LIMIT_FUNCTION not in output:
        raise GateVerificationError(
            f"the diagnostic names no {OVER_LIMIT_FUNCTION!r}:\n" + output
        )
    if IN_LIMIT_FUNCTION in output:
        raise GateVerificationError(
            f"the diagnostic names the in-limit example {IN_LIMIT_FUNCTION!r}:\n"
            + output
        )
    return [
        f"in-limit example accepted at limit {limit}",
        f"over-limit example reported at {scored} (limit {stated_limit})",
    ]


app = cyclopts.App(
    name="verify-windows-complexity-gate",
    help=__doc__.splitlines()[0],
)


@app.default
def verify_command(
    *, whitaker: str = "whitaker", target: str = "x86_64-pc-windows-msvc"
) -> int:
    """Verify both directions and report the evidence.

    Parameters
    ----------
    whitaker : str
        The Whitaker launcher to run.
    target : str
        The target triple to check at.

    Returns
    -------
    int
        ``0`` when both directions behaved as specified, ``1`` otherwise.
    """
    try:
        lines = verify(whitaker=whitaker, target=target)
    except GateVerificationError as error:
        print(f"windows complexity gate: {error}")
        return 1
    for line in lines:
        print(f"windows complexity gate: {line}")
    return 0



if __name__ == "__main__":
    app()