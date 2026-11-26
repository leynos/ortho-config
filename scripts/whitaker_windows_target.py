"""Keep Whitaker's suite toolchain able to build the Windows target.

Whitaker's lints run inside a compiler driver, so the suite only scores the
``cfg`` surface that compiler was asked to build. The host pass in
``make lint-whitaker`` therefore scores no ``#[cfg(windows)]`` item at all: the
compiler removes those before any lint sees them. ``make
lint-whitaker-windows`` asks for the Windows target instead, which keeps them.

That cross-check needs the Windows target's ``rust-std`` component in the
toolchain the suite runs on. That toolchain is the installer's, not the one
``rust-toolchain.toml`` pins, so the component cannot be added by a bare
``rustup target add``: the channel has to be read from the installer first.
This module owns that lookup, its ``check`` and ``install`` verbs, and nothing
else, so the Makefile keeps expressing *what* the gate needs rather than how
the installer happens to name its toolchain.

The lookup reads ``whitaker-installer install --dry-run``, which prints the
configuration it would act on and writes nothing. Its ``Toolchain:`` line is
the same value the real install uses, and naming it explicitly means a change
in that output fails this gate loudly instead of silently checking the wrong
compiler. The installer reports on standard error, so both streams are read:
which one carries the line is not a contract worth depending on.

Run via ``make install-whitaker-windows-target`` or
``make check-whitaker-windows-target``.
"""

from __future__ import annotations

import argparse
import subprocess  # noqa: S404 - the installer and rustup are the subject.
import typing as typ

#: The line ``whitaker-installer install --dry-run`` prints its toolchain on.
TOOLCHAIN_PREFIX: typ.Final[str] = "Toolchain: "

#: The default installer executable. The `whitaker` wrapper resolves the same
#: name beside itself or on `PATH`, so a machine that can run the suite can
#: run this.
DEFAULT_INSTALLER: typ.Final[str] = "whitaker-installer"

#: The default target triple. Named here as well as in the Makefile because the
#: module is usable on its own, and the two are held equal by the contract test
#: `tests/workflow_contracts/windows_complexity_gate_test.py`.
DEFAULT_TARGET: typ.Final[str] = "x86_64-pc-windows-msvc"


class ToolchainLookupError(RuntimeError):
    """Raised when the suite toolchain cannot be read from the installer."""


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


def suite_toolchain(installer: str) -> str:
    """Return the channel Whitaker's installer builds its suite for.

    Parameters
    ----------
    installer : str
        The installer executable to query.

    Returns
    -------
    str
        The toolchain channel the suite runs on.

    Raises
    ------
    ToolchainLookupError
        If the installer cannot be run, or its output carries no channel.
    """
    try:
        completed = subprocess.run(  # noqa: S603 - a fixed argv, no shell.
            [installer, "install", "--dry-run"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise ToolchainLookupError(
            f"could not read {installer!r}'s configured toolchain: {error}"
        ) from error
    return parse_toolchain(completed.stdout + completed.stderr)


def installed_targets(channel: str) -> set[str]:
    """Return the target triples ``rustup`` has installed for *channel*.

    Parameters
    ----------
    channel : str
        The toolchain channel to ask about.

    Returns
    -------
    set of str
        The installed triples, empty when `rustup` reports none.
    """
    completed = subprocess.run(  # noqa: S603 - a fixed argv, no shell.
        ["rustup", "target", "list", "--installed", "--toolchain", channel],
        check=True,
        capture_output=True,
        text=True,
    )
    return set(completed.stdout.split())


def install_target(channel: str, target: str) -> None:
    """Add *target*'s standard library to *channel*.

    Parameters
    ----------
    channel : str
        The toolchain channel to extend.
    target : str
        The target triple to add.
    """
    subprocess.run(  # noqa: S603 - a fixed argv, no shell.
        ["rustup", "target", "add", "--toolchain", channel, target],
        check=True,
    )


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
        ``0`` on success, ``1`` when the target is missing (``check`` only).

    Raises
    ------
    ToolchainLookupError
        If the suite toolchain cannot be read.
    """
    arguments = build_parser().parse_args(argv)
    channel = suite_toolchain(arguments.installer)
    if arguments.verb == "install":
        install_target(channel, arguments.target)
        print(f"whitaker windows target: {arguments.target} installed for {channel}")
        return 0
    if arguments.target in installed_targets(channel):
        print(f"whitaker windows target: {arguments.target} is installed for {channel}")
        return 0
    print(
        f"whitaker windows target: {arguments.target} is missing from {channel}.\n"
        f"Run `make install-whitaker-windows-target` to add it.",
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())