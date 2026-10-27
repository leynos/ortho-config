"""Evaluate the Makefile's dev-fast routing for build-standard contracts.

Reading a recipe's text proves little about the command it runs: the flags are
Make variables, the executable is a variable, and the platform conditionals
choose between them. These helpers ask Make itself. ``make --dry-run`` expands
every variable and takes every conditional exactly as a real run would, and
prints the commands instead of running them, so a contract can assert on the
command line that would have reached Cargo.

Two things are substituted so the answer is about the Makefile and not the
machine: ``CARGO`` becomes ``probe-cargo``, which proves a recipe goes through
``$(CARGO)`` (a hard-coded ``cargo`` would survive the substitution and be
caught), and the host platform is passed as ``BUILD_HOST_OS`` so one Linux
machine can read the macOS and Windows routes.

The Cargo configuration is read here too, because the contract that matters is
agreement between the two: a bare ``cargo`` invocation and a Make recipe must
build the same way on the same host.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import dataclasses
import pathlib
import shlex
import shutil
import subprocess  # noqa: S404 - the contract drives make, which is the subject.
import tomllib
import typing as typ

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
CONFIG_PATH: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".cargo" / "config.toml"

#: The executables substituted for the real ones. Neither exists, which is the
#: point: a dry run never executes them.
PROBE_CARGO: typ.Final[str] = "probe-cargo"
PROBE_WHITAKER: typ.Final[str] = "probe-whitaker"

#: The flags the development standard adds. A route that must not carry the
#: standard is checked against every flag, so none can arrive through an
#: excluded route.
THREADS_FLAG: typ.Final[str] = "-Zthreads=8"
BACKEND_FLAG: typ.Final[str] = "-Zcodegen-backend=cranelift"
DEV_FAST_CFG_FLAG: typ.Final[str] = "--cfg=ortho_config_dev_fast"
CHECK_CFG_FLAG: typ.Final[str] = "--check-cfg=cfg(ortho_config_dev_fast)"
LINKER_FLAG: typ.Final[str] = "-Clink-arg=-fuse-ld=mold"
STANDARD_FLAGS: typ.Final[frozenset[str]] = frozenset(
    {THREADS_FLAG, BACKEND_FLAG, DEV_FAST_CFG_FLAG, CHECK_CFG_FLAG, LINKER_FLAG}
)

#: The one target table that applies on Linux, and so the only place the linker
#: may appear. ``cfg(unix)`` would also reach macOS, which ships no ``mold``.
LINUX_CFG: typ.Final[str] = 'cfg(target_os = "linux")'

#: The preflight script the build-tools target runs.
PREFLIGHT_SCRIPT: typ.Final[str] = "scripts/check-build-tools.sh"


@dataclasses.dataclass(frozen=True)
class Host:
    """A development host, as the Makefile and Cargo each name it.

    Attributes
    ----------
    make_os : str
        The value of ``BUILD_HOST_OS`` (``uname -s``, or ``Windows_NT``).
    triple : str
        The target triple Cargo matches ``[target.*]`` tables against when it
        builds natively on this host.
    """

    make_os: str
    triple: str

    def variables(self) -> list[str]:
        """Return the command-line assignments that make Make see this host.

        ``OS`` is assigned as well, because the Makefile branches on
        ``Windows_NT`` there and on nothing else.

        Returns
        -------
        list of str
            ``NAME=value`` arguments for ``make``.
        """
        os_value = "Windows_NT" if self.make_os == "Windows_NT" else "Unix"
        return [f"BUILD_HOST_OS={self.make_os}", f"OS={os_value}"]


#: Every host the standard distinguishes: Linux, which gets the linker, on both
#: architectures it is built for, and the two platforms that keep their own.
HOSTS: typ.Final[dict[str, Host]] = {
    "linux_x86_64": Host("Linux", "x86_64-unknown-linux-gnu"),
    "linux_aarch64": Host("Linux", "aarch64-unknown-linux-gnu"),
    "macos_arm64": Host("Darwin", "aarch64-apple-darwin"),
    "windows_x86_64": Host("Windows_NT", "x86_64-pc-windows-msvc"),
}


def dry_run(target: str, *variables: str, host: Host | None = None) -> list[str]:
    """Return the commands ``make`` would run for *target*, one per line.

    Parameters
    ----------
    target : str
        The Make target to expand.
    *variables : str
        Extra ``NAME=value`` arguments, such as ``BUILD_STANDARD=0``.
    host : Host or None
        The host to evaluate for; ``None`` leaves Make to detect this one.

    Returns
    -------
    list of str
        The expanded recipe lines, blank lines dropped.

    Raises
    ------
    AssertionError
        If ``make`` exits non-zero. A route that cannot be expanded must fail
        the contract that asked, not read as an empty one.
    """
    command = [
        make_executable(),
        "--dry-run",
        "--no-print-directory",
        "--silent",
        target,
        f"CARGO={PROBE_CARGO}",
        f"WHITAKER={PROBE_WHITAKER}",
        *(host.variables() if host else []),
        *variables,
    ]
    result = subprocess.run(  # noqa: S603 - resolved executable, fixed argument vector, no shell.
        command,
        cwd=REPOSITORY_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, (
        f"{' '.join(command)} exited {result.returncode}: {result.stderr.strip()}"
    )
    return [line for line in result.stdout.splitlines() if line.strip()]


def make_executable() -> str:
    """Return the resolved Make executable required by these contracts."""
    executable = shutil.which("make")
    assert executable is not None, "make is required to evaluate the Makefile contracts"
    return str(pathlib.Path(executable).resolve())


def shell_tokens(line: str) -> list[str]:
    """Split an expanded recipe line, tolerating comments and malformed quotes.

    Make's dry-run output is shell text. Valid lines use ``shlex`` so quoted
    values stay together. A malformed quoting example should not crash a
    contract helper; its whitespace fallback is deliberately best-effort.
    Full comment lines are ignored before either parser can mistake their
    contents for a command or assignment.
    """
    if line.lstrip().startswith("#"):
        return []
    try:
        return shlex.split(line, comments=True)
    except ValueError:
        return line.split()


def cargo_lines(lines: list[str]) -> list[str]:
    """Return the lines that invoke Cargo, through the substituted executable.

    Parameters
    ----------
    lines : list of str
        Expanded recipe lines.

    Returns
    -------
    list of str
        Those whose tokens include ``probe-cargo``.

    Examples
    --------
    >>> cargo_lines(['RUSTFLAGS="" probe-cargo build --lib', 'echo done'])
    ['RUSTFLAGS="" probe-cargo build --lib']
    """
    return [line for line in lines if PROBE_CARGO in shell_tokens(line)]


def hard_coded_cargo(lines: list[str]) -> list[str]:
    """Return the lines that name the real ``cargo`` as a command word.

    Parameters
    ----------
    lines : list of str
        Expanded recipe lines.

    Returns
    -------
    list of str
        Lines where ``cargo`` stands as its own token, which can only happen
        when a recipe bypassed ``$(CARGO)``.

    Examples
    --------
    >>> hard_coded_cargo(['probe-cargo test', 'cargo test', '# the cargo output'])
    ['cargo test']
    """
    return [line for line in lines if "cargo" in shell_tokens(line)]


def assigned_rustflags(line: str) -> list[str] | None:
    """Return the ``RUSTFLAGS`` a command line assigns, or ``None``.

    Parameters
    ----------
    line : str
        One expanded recipe line.

    Returns
    -------
    list of str or None
        The assigned value split into flags, ``[]`` for an assignment of the
        empty string, and ``None`` when the line assigns nothing. The
        distinction matters: an empty assignment displaces the configuration,
        an absent one leaves it in force.

    Examples
    --------
    >>> assigned_rustflags('RUSTFLAGS="-D warnings -Zthreads=8" probe-cargo test')
    ['-D', 'warnings', '-Zthreads=8']
    >>> assigned_rustflags('RUSTFLAGS="" probe-cargo build')
    []
    >>> assigned_rustflags('probe-cargo build') is None
    True
    """
    for token in shell_tokens(line):
        if token.startswith("RUSTFLAGS="):
            return token.removeprefix("RUSTFLAGS=").split()
    return None


def development_flags(flags: list[str]) -> frozenset[str]:
    """Return the flags of the development standard among *flags*.

    Parameters
    ----------
    flags : list of str
        Split ``RUSTFLAGS`` tokens.

    Returns
    -------
    frozenset of str
        Those that belong to the standard. Warning policy is not among them.

    Examples
    --------
    >>> sorted(development_flags(['-D', 'warnings', '-Zthreads=8']))
    ['-Zthreads=8']
    """
    return frozenset(flags) & STANDARD_FLAGS


def configured_flags(host: Host) -> frozenset[str]:
    """Return the ``rustflags`` Cargo applies when building natively on *host*.

    Cargo selects one source rather than merging: a matching ``[target.*]``
    table that sets ``rustflags`` replaces ``[build] rustflags`` outright. This
    reads the configuration the same way, so the answer is what a bare
    ``cargo build`` would pass, not a union of everything the file mentions.

    Parameters
    ----------
    host : Host
        The host whose native build is read.

    Returns
    -------
    frozenset of str
        The flags of the selected source.
    """
    document = tomllib.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    tables = document.get("target", {})
    selected = tables.get(host.triple, {}).get("rustflags")
    if selected is None and "linux" in host.triple:
        selected = tables.get(LINUX_CFG, {}).get("rustflags")
    if selected is None:
        selected = document.get("build", {}).get("rustflags", [])
    return frozenset(selected)
