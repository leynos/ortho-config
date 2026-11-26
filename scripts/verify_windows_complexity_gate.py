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

import argparse
import os
import pathlib
import re
import shlex
import subprocess  # noqa: S404 - the gate is an external command, the subject.
import typing as typ

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
    return shlex.split(launcher, posix=os.name != "nt")


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
    try:
        return subprocess.run(  # noqa: S603 - a fixed argv, no shell.
            gate_command(whitaker=whitaker, target=target, over_limit=over_limit),
            check=False,
            capture_output=True,
            text=True,
            env=gate_environment(),
            timeout=INVOCATION_DEADLINE_SECONDS,
        )
    except subprocess.TimeoutExpired as expired:
        raise GateVerificationError(
            f"the {target} probe did not finish within "
            f"{INVOCATION_DEADLINE_SECONDS}s"
        ) from expired
    except OSError as error:
        raise GateVerificationError(f"could not run {whitaker!r}: {error}") from error


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


def build_parser() -> argparse.ArgumentParser:
    """Return the command-line parser for this module.

    Returns
    -------
    argparse.ArgumentParser
        A parser accepting the launcher and the target triple.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--whitaker", default="whitaker")
    parser.add_argument("--target", default="x86_64-pc-windows-msvc")
    return parser


def main(argv: typ.Sequence[str] | None = None) -> int:
    """Verify both directions and report the evidence.

    Parameters
    ----------
    argv : sequence of str or None
        Arguments to parse; ``None`` reads ``sys.argv``.

    Returns
    -------
    int
        ``0`` when both directions behaved as specified, ``1`` otherwise.
    """
    arguments = build_parser().parse_args(argv)
    try:
        lines = verify(whitaker=arguments.whitaker, target=arguments.target)
    except GateVerificationError as error:
        print(f"windows complexity gate: {error}")
        return 1
    for line in lines:
        print(f"windows complexity gate: {line}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())