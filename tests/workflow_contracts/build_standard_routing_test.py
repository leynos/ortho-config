"""Contract for how the Makefile routes the development build standard.

The standard (the parallel frontend and ``mold``, both in
``.cargo/config.toml``) reaches a build by one of two routes, and each can fail
silently:

- A bare ``cargo`` invocation reads the configuration file. That route cannot
  fail in a Make recipe.
- A recipe that assigns ``RUSTFLAGS`` replaces every ``rustflags`` source in the
  file, so a gate that assigned only ``-D warnings`` would build without the
  standard and still go green. The recipe has to restate the flags.

So the contract is evaluated, not read. Each case expands a target with
``make --dry-run CARGO=probe-cargo`` and judges the command line that would have
reached Cargo: positive cases fail when a development invocation loses a flag,
negative cases fail when release, the MSRV check or Whitaker gains one. The
flags are compared with the configuration file for each host, because the
agreement between the two is the property that matters; a flag restated in
Make but absent from the file (or the reverse) is a build that differs between
``cargo build`` and ``make build``.

Case identifiers are distinct (``make_test_linux_x86_64`` and so on) so each
shows up by name in a collection listing and none can be dropped by a
parametrisation that shares one.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import subprocess  # noqa: S404 - one case runs a real make, which is the subject.
import tomllib
import typing as typ

import pytest
from build_standard_make import (
    BACKEND_FLAG,
    CONFIG_PATH,
    HOSTS,
    LINKER_FLAG,
    LINUX_CFG,
    PREFLIGHT_SCRIPT,
    PROBE_WHITAKER,
    REPOSITORY_ROOT,
    STANDARD_FLAGS,
    THREADS_FLAG,
    Host,
    assigned_rustflags,
    cargo_lines,
    configured_flags,
    development_flags,
    dry_run,
    hard_coded_cargo,
)

#: Each development invocation: the case name, the target, and a word that
#: identifies the Cargo subcommand on the line under test. ``lint-clippy`` runs
#: two Cargo commands and each needs its own proof, as do ``doc`` and ``clippy``.
DEVELOPMENT_ROUTES: typ.Final[list[tuple[str, str, str]]] = [
    ("make_test", "test", "test"),
    ("make_typecheck", "typecheck", "check"),
    ("make_lint_clippy_doc", "lint-clippy", "doc"),
    ("make_lint_clippy_clippy", "lint-clippy", "clippy"),
    ("make_build", "build", "build"),
]

#: Routes that deny warnings as well as carrying the standard. ``build`` keeps
#: the caller's warning policy and imposes none.
DENIES_WARNINGS: typ.Final[frozenset[str]] = frozenset(
    {"make_test", "make_typecheck", "make_lint_clippy_doc", "make_lint_clippy_clippy"}
)

DEVELOPMENT_CASES: typ.Final[list[object]] = [
    pytest.param(case, target, subcommand, host_name, id=f"{case}_{host_name}")
    for case, target, subcommand in DEVELOPMENT_ROUTES
    for host_name in HOSTS
]


def _line_for(
    target: str,
    subcommand: str,
    *,
    variables: tuple[str, ...] = (),
    host: Host | None = None,
) -> str:
    """Return the one Cargo line of *target* that runs *subcommand*.

    Exactly one is required: zero would mean the route no longer reaches Cargo
    (a vacuous pass for every assertion after it), and two would mean the case
    cannot say which it judged.
    """
    lines = dry_run(target, *variables, host=host)
    matching = [line for line in cargo_lines(lines) if f"probe-cargo {subcommand} " in f"{line} "]
    assert len(matching) == 1, (
        f"`make {target}` has {len(matching)} Cargo lines running {subcommand!r}: {lines}"
    )
    return matching[0]


@pytest.mark.parametrize(("case", "target", "subcommand", "host_name"), DEVELOPMENT_CASES)
def test_development_routes_restate_the_standard_for_their_host(
    case: str, target: str, subcommand: str, host_name: str
) -> None:
    """A development invocation carries exactly the flags the file gives its host.

    Equality, not containment: an extra development flag is as much a
    disagreement between ``cargo build`` and ``make build`` as a missing one.
    """
    host = HOSTS[host_name]
    line = _line_for(target, subcommand, host=host)
    flags = assigned_rustflags(line)
    assert flags is not None, f"{case}: no RUSTFLAGS assignment on {line!r}"
    assert development_flags(flags) == configured_flags(host) & STANDARD_FLAGS, (
        f"{case} on {host_name} restates {sorted(development_flags(flags))}; the configuration "
        f"gives {sorted(configured_flags(host) & STANDARD_FLAGS)}"
    )
    assert ("-D" in flags and "warnings" in flags) == (case in DENIES_WARNINGS), (
        f"{case}: warning policy on {line!r}"
    )


ROUTE_CASES: typ.Final[list[object]] = [pytest.param(*row, id=row[0]) for row in DEVELOPMENT_ROUTES]


@pytest.mark.parametrize(("case", "target", "subcommand"), ROUTE_CASES)
def test_development_routes_go_through_the_cargo_variable(case: str, target: str, subcommand: str) -> None:
    """The executable is injectable: no recipe names the real ``cargo``."""
    lines = dry_run(target, host=HOSTS["linux_x86_64"])
    assert _line_for(target, subcommand, host=HOSTS["linux_x86_64"]), case
    assert not hard_coded_cargo(lines), f"{case}: `make {target}` names cargo directly: {hard_coded_cargo(lines)}"


def test_the_standard_is_the_whole_standard_on_each_platform() -> None:
    """Pins each platform to its exact flags, so a loosened table cannot hide.

    The parametrised cases compare Make with the file, which agree trivially
    if both lose a flag together. This case names the standard itself: Linux
    gets the parallel frontend and the linker, on either architecture, and the
    two platforms that ship no ``mold`` get the frontend alone.
    """
    for host_name in ("linux_x86_64", "linux_aarch64"):
        assert configured_flags(HOSTS[host_name]) == {THREADS_FLAG, LINKER_FLAG}, host_name
    for host_name in ("macos_arm64", "windows_x86_64"):
        assert configured_flags(HOSTS[host_name]) == {THREADS_FLAG}, host_name


def test_the_linker_is_never_named_beyond_linux() -> None:
    """``mold`` ships for Linux alone, so no other host's route may carry it."""
    for host_name in ("macos_arm64", "windows_x86_64"):
        host = HOSTS[host_name]
        for _case, target, subcommand in DEVELOPMENT_ROUTES:
            flags = assigned_rustflags(_line_for(target, subcommand, host=host)) or []
            assert LINKER_FLAG not in flags, f"{target} on {host_name} names {LINKER_FLAG}"


def test_the_linker_is_scoped_to_the_linux_table_alone() -> None:
    """The configuration names ``mold`` under ``cfg(target_os = "linux")`` and nowhere else.

    ``cfg(unix)`` would also reach macOS and an unconditional entry would reach
    Windows, so the table keys are read, not searched for.
    """
    document = tomllib.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    carriers = [
        name
        for name, table in [("[build]", document.get("build", {})), *(
            (f"[target.{key}]", value) for key, value in document.get("target", {}).items()
        )]
        if LINKER_FLAG in table.get("rustflags", [])
    ]
    assert carriers == [f"[target.{LINUX_CFG}]"], f"the linker is named by {carriers}"


def test_cranelift_is_refused_until_the_measurement_is_cleared() -> None:
    """No file selects a codegen backend while the recorded Cranelift exception stands.

    Cranelift was measured against this repository's own suite on the pinned
    nightly and refused: a panic does not unwind through Cranelift-compiled
    frames, and the suite catches panics (``docs/developers-guide.md``,
    "Cranelift qualification"). The estate's rule accepts either an adopted
    backend or a recorded exception, and a recorded exception is only honest if
    nothing quietly selects the backend anyway. This reads every place a
    selection could live: a profile key or an ``[unstable]`` table in the Cargo
    configuration, a ``-Zcodegen-backend`` token in any ``rustflags`` source, a
    Makefile variable, and a ``CARGO_PROFILE_*_CODEGEN_BACKEND`` assignment in a
    workflow.

    To adopt a backend, re-measure on the new nightly, clear the exception in
    the guide in the same change, and replace this contract with one that holds
    the adopted route.
    """
    document = tomllib.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    assert "unstable" not in document, "an [unstable] table enables a backend selection"
    profiles = document.get("profile", {})
    assert not any("codegen-backend" in table for table in profiles.values()), (
        "a [profile] table selects a codegen backend"
    )
    flags = [
        flag
        for table in [document.get("build", {}), *document.get("target", {}).values()]
        for flag in table.get("rustflags", [])
    ]
    assert not any("codegen-backend" in flag for flag in flags), f"rustflags select a backend: {flags}"
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    assert "codegen-backend" not in makefile, "the Makefile names a codegen backend"
    for workflow in sorted((REPOSITORY_ROOT / ".github" / "workflows").glob("*.yml")):
        text = workflow.read_text(encoding="utf-8")
        assert "CODEGEN_BACKEND" not in text, f"{workflow.name} selects a codegen backend"
        assert BACKEND_FLAG not in text, f"{workflow.name} names {BACKEND_FLAG}"


# -- exclusions: release, MSRV, Whitaker, and the explicit opt-out ------------------


def test_make_release_takes_none_of_the_standard() -> None:
    """A release build assigns ``RUSTFLAGS`` and assigns nothing development.

    The assignment must be present: with none, Cargo reads the configuration
    and the release build would link with mold and select Cranelift.
    """
    (line,) = [
        line for line in cargo_lines(dry_run("release", host=HOSTS["linux_x86_64"])) if "build" in line
    ]
    flags = assigned_rustflags(line)
    assert flags is not None, f"release leaves the configuration in force: {line!r}"
    assert not development_flags(flags), f"release carries {sorted(development_flags(flags))}"
    assert "--release" in line, f"release no longer builds the release profile: {line!r}"


def test_make_release_does_not_reach_the_build_tools_preflight() -> None:
    """A release build must not fail for want of a linker it will not use."""
    assert PREFLIGHT_SCRIPT not in dry_run("release", host=HOSTS["linux_x86_64"])


def test_make_lint_whitaker_takes_none_of_the_standard() -> None:
    """Whitaker keeps its own toolchain and a fixed, development-free ``RUSTFLAGS``."""
    lines = dry_run("lint-whitaker", host=HOSTS["linux_x86_64"])
    (line,) = [line for line in lines if PROBE_WHITAKER in line]
    flags = assigned_rustflags(line)
    assert flags == ["-D", "warnings"], f"Whitaker's RUSTFLAGS is {flags!r}, not `-D warnings` alone"
    assert not cargo_lines(lines), f"lint-whitaker reaches Cargo directly: {cargo_lines(lines)}"
    assert PREFLIGHT_SCRIPT not in lines, "lint-whitaker needs none of the standard's tools"


def test_make_msrv_check_is_a_stable_check_without_the_standard() -> None:
    """The MSRV lane checks every target on the MSRV toolchain and runs no tests.

    A stable compiler refuses ``-Zthreads`` and the backend flag, so the
    assignment must displace the configuration rather than inherit it.
    """
    lines = dry_run("msrv-check", host=HOSTS["linux_x86_64"])
    (line,) = cargo_lines(lines)
    flags = assigned_rustflags(line)
    assert flags is not None, f"msrv-check leaves the nightly-only flags in force: {line!r}"
    assert not development_flags(flags), f"msrv-check carries {sorted(development_flags(flags))}"
    tokens = line.split()
    assert tokens[tokens.index("probe-cargo") + 1].startswith("+"), f"no `+toolchain` selection: {line!r}"
    assert "check" in tokens and "--all-targets" in tokens, f"msrv-check is not an all-targets check: {line!r}"
    assert not {"test", "nextest", "llvm-cov"} & set(tokens), f"msrv-check runs tests: {line!r}"


OPT_OUT_CASES: typ.Final[list[object]] = [
    pytest.param(*row, id=row[0]) for row in DEVELOPMENT_ROUTES if row[0] != "make_build"
]


@pytest.mark.parametrize(("case", "target", "subcommand"), OPT_OUT_CASES)
def test_the_opt_out_removes_the_standard_and_nothing_else(case: str, target: str, subcommand: str) -> None:
    """``BUILD_STANDARD=0`` is how a non-nightly toolchain runs the gates."""
    line = _line_for(target, subcommand, variables=("BUILD_STANDARD=0",), host=HOSTS["linux_x86_64"])
    flags = assigned_rustflags(line)
    assert flags is not None
    assert not development_flags(flags), f"{case} keeps {sorted(development_flags(flags))} with the standard off"
    assert "-D" in flags, f"{case} stopped denying warnings with the standard off"


# -- the preflight: ordering and failure propagation -----------------------------------


PREFLIGHT_TARGETS: typ.Final[list[object]] = [
    pytest.param(target, id=f"preflight_{target.replace('-', '_')}")
    for target in ("test", "typecheck", "lint-clippy", "lint", "build")
]


@pytest.mark.parametrize("target", PREFLIGHT_TARGETS)
def test_standard_routes_reach_the_preflight_before_compiling(target: str) -> None:
    """Every standard route runs the build-tools check ahead of its first Cargo line."""
    lines = dry_run(target, host=HOSTS["linux_x86_64"])
    assert PREFLIGHT_SCRIPT in lines, f"`make {target}` never reaches the preflight: {lines}"
    first_cargo = next(index for index, line in enumerate(lines) if cargo_lines([line]))
    assert lines.index(PREFLIGHT_SCRIPT) < first_cargo, f"`make {target}` compiles before its preflight: {lines}"


def test_the_preflight_is_a_noop_when_the_standard_is_off() -> None:
    """With ``BUILD_STANDARD=0`` the preflight prints a note and checks nothing."""
    lines = dry_run("check-build-tools", "BUILD_STANDARD=0")
    assert PREFLIGHT_SCRIPT not in lines
    assert any("the build standard is off" in line for line in lines), lines


def test_a_failing_preflight_stops_the_build_before_cargo(tmp_path: pathlib.Path) -> None:
    """A missing prerequisite aborts with an actionable error and never compiles.

    The pinned mold version is pointed at one nothing installs, so the check
    fails on any machine whether or not mold is present. ``CARGO`` is a script
    that leaves a marker, so "never compiles" is observed rather than assumed.
    """
    marker = tmp_path / "cargo-ran"
    fake_cargo = tmp_path / "fake-cargo"
    fake_cargo.write_text(f"#!/bin/sh\ntouch '{marker}'\n", encoding="utf-8")
    fake_cargo.chmod(0o755)
    wrong_pin = tmp_path / "VERSION"
    wrong_pin.write_text("0.0.1\n", encoding="utf-8")
    result = subprocess.run(  # noqa: S603, S607 - fixed argument vector, no shell.
        ["make", "typecheck", f"CARGO={fake_cargo}", f"MOLD_VERSION_FILE={wrong_pin}"],
        cwd=REPOSITORY_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0, f"make typecheck passed with an unsatisfiable pin: {result.stdout}"
    assert not marker.exists(), "Cargo ran although the preflight failed"
    assert "make install-build-tools" in result.stderr, (
        f"the error does not say how to fix it: {result.stderr!r}"
    )
