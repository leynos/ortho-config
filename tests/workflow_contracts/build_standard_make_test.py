"""Contracts for errors at the Make and Cargo-configuration boundaries.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import subprocess
import tomllib

import pytest

import build_standard_make
from build_standard_make import (
    CargoConfigurationError,
    HOSTS,
    LINUX_CFG,
    MakeDryRunError,
    configured_flags,
    dry_run,
    load_cargo_configuration,
)


def test_missing_make_has_a_domain_error(monkeypatch: pytest.MonkeyPatch) -> None:
    """A missing Make executable crosses the command boundary as an error."""
    monkeypatch.setattr(build_standard_make.shutil, "which", lambda _name: None)

    with pytest.raises(MakeDryRunError, match="make is required"):
        dry_run("test")


def test_process_start_errors_keep_their_cause(monkeypatch: pytest.MonkeyPatch) -> None:
    """An OS-level start failure is explicit and retains its original cause."""
    monkeypatch.setattr(build_standard_make, "make_executable", lambda: "make")

    def cannot_start(*_args: object, **_kwargs: object) -> subprocess.CompletedProcess[str]:
        """Stand in for a Make process the host cannot start."""
        raise FileNotFoundError("make vanished")

    monkeypatch.setattr(build_standard_make.subprocess, "run", cannot_start)

    with pytest.raises(MakeDryRunError, match="could not start") as error:
        dry_run("test")

    assert isinstance(error.value.__cause__, FileNotFoundError)


def test_nonzero_make_exit_includes_exit_code_and_stderr(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A failed dry run cannot be mistaken for an empty recipe."""
    monkeypatch.setattr(build_standard_make, "make_executable", lambda: "make")

    def rejected_target(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
        """Stand in for Make rejecting the requested target."""
        return subprocess.CompletedProcess(command, 2, "", "unknown target")

    monkeypatch.setattr(build_standard_make.subprocess, "run", rejected_target)

    with pytest.raises(MakeDryRunError, match="exited 2: unknown target"):
        dry_run("missing-target")


def test_config_reader_propagates_file_errors(tmp_path: pathlib.Path) -> None:
    """A missing configuration file remains an inspectable I/O failure."""
    with pytest.raises(FileNotFoundError):
        load_cargo_configuration(tmp_path / "missing.toml")


def test_config_reader_propagates_toml_errors(tmp_path: pathlib.Path) -> None:
    """Malformed configuration remains an inspectable TOML parse failure."""
    path = tmp_path / "config.toml"
    path.write_text("[build\nrustflags = []\n", encoding="utf-8")

    with pytest.raises(tomllib.TOMLDecodeError):
        load_cargo_configuration(path)


def test_configured_flags_selects_from_an_injected_document() -> None:
    """The pure query reads only its parsed input and keeps Cargo's precedence."""
    document: dict[str, object] = {
        "build": {"rustflags": ["-Zthreads=8"]},
        "target": {LINUX_CFG: {"rustflags": ["-Clink-arg=-fuse-ld=mold"]}},
    }

    assert configured_flags(HOSTS["linux_x86_64"], document) == {
        "-Clink-arg=-fuse-ld=mold"
    }


def test_configured_flags_rejects_a_malformed_target_table() -> None:
    """A malformed configuration shape cannot silently remove all flags."""
    with pytest.raises(CargoConfigurationError, match="must be a TOML table"):
        configured_flags(HOSTS["linux_x86_64"], {"target": []})
