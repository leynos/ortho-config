"""Lading compatibility patches for publish preflight exclusions.

This module is loaded automatically by Python when present on ``PYTHONPATH``.
The hook extends the upstream Lading CLI so configuration files may define a
``[preflight]`` section describing crates to exclude from pre-flight ``cargo
 test`` invocations. Until the upstream project gains native support we inject
behaviour here so the `publish-check` Makefile target can skip costly cucumber
integration tests during release validation.
"""

from __future__ import annotations

import functools
import os
from pathlib import Path
from typing import Iterable

try:  # pragma: no cover - defensive import for Python < 3.11
    import tomllib  # type: ignore[attr-defined]
except ModuleNotFoundError:  # pragma: no cover - fallback for older Pythons
    import tomli as tomllib  # type: ignore[assignment]

_PRE_FLIGHT_ENV = "LADING_WORKSPACE_ROOT"


def _resolve_workspace_root(raw_root: Path | str | None) -> Path | None:
    """Normalise the workspace root value to a :class:`Path`.

    ``lading`` exposes the workspace location via ``LADING_WORKSPACE_ROOT``.
    Some call-sites (for example unit tests) may import :mod:`lading.commands`
    without populating the environment, so the helper accepts an explicit path
    override. Any falsy values return :data:`None` so callers can fall back to
    other sources.
    """

    if not raw_root:
        return None
    return Path(raw_root)


def _read_lading_config(workspace_root: Path) -> dict | None:
    """Return parsed ``lading.toml`` configuration for ``workspace_root``."""

    config_path = workspace_root / "lading.toml"
    try:
        config_text = config_path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return None
    try:
        return tomllib.loads(config_text)
    except (tomllib.TOMLDecodeError, AttributeError):
        # Invalid configuration falls back to running the default suite. The
        # publish command will report a structured error when it attempts to
        # parse the file for its own options.
        return None


def _normalize_exclude_list(raw_excludes) -> tuple[str, ...]:
    """Normalise ``test_exclude`` entries to a tuple of crate names."""

    if isinstance(raw_excludes, str):
        iterable: Iterable[str] | None = (raw_excludes,)
    elif isinstance(raw_excludes, Iterable):
        iterable = (item for item in raw_excludes if isinstance(item, str))
    else:
        iterable = None
    if iterable is None:
        return ()
    excludes: list[str] = []
    for item in iterable:
        candidate = item.strip()
        if candidate:
            excludes.append(candidate)
    return tuple(excludes)


@functools.lru_cache(maxsize=8)
def _load_preflight_test_excludes(workspace_root: Path | None) -> tuple[str, ...]:
    """Return crate names that pre-flight ``cargo test`` runs should skip."""

    if workspace_root is None:
        return ()
    parsed = _read_lading_config(workspace_root)
    if parsed is None:
        return ()
    raw_preflight = parsed.get("preflight")
    if not isinstance(raw_preflight, dict):
        return ()
    raw_excludes = raw_preflight.get("test_exclude", ())
    return _normalize_exclude_list(raw_excludes)


def _patch_configuration_validation() -> None:
    """Allow ``[preflight]`` in ``lading.toml`` without raising errors."""

    try:
        from lading import config as config_module
    except Exception:  # pragma: no cover - lading unavailable
        return

    original_validate = config_module._validate_mapping_keys

    def patched_validate(mapping, allowed_keys, context):  # type: ignore[override]
        if context == "configuration section":
            allowed_keys = set(allowed_keys) | {"preflight"}
        return original_validate(mapping, allowed_keys, context)

    config_module._validate_mapping_keys = patched_validate  # type: ignore[assignment]


def _build_cargo_error_message(
    subcommand: str, exit_code: int, stdout: str, stderr: str
) -> str:
    """Return a consistent failure message for cargo pre-flight commands."""

    message = f"Pre-flight cargo {subcommand} failed with exit code {exit_code}"
    detail = (stderr or stdout).strip()
    if detail:
        message = f"{message}: {detail}"
    return message


def _patched_run_cargo_preflight(
    workspace_root,
    subcommand,
    *,
    runner,
    extra_args=None,
    publish_module,
):
    workspace_path = Path(workspace_root)
    arguments: list[str] = list(extra_args or ("--workspace", "--all-targets"))
    if subcommand == "test":
        excludes = _load_preflight_test_excludes(workspace_path)
        for crate in excludes:
            arguments.extend(("--exclude", crate))
    exit_code, stdout, stderr = runner(
        ("cargo", subcommand, *arguments),
        cwd=workspace_path,
    )
    if exit_code != 0:
        message = _build_cargo_error_message(subcommand, exit_code, stdout, stderr)
        raise publish_module.PublishPreflightError(message)


def _patch_preflight_runner() -> None:
    """Inject ``--exclude`` flags for configured pre-flight test crates."""

    try:
        from lading.commands import publish as publish_module
    except Exception:  # pragma: no cover - lading unavailable
        return

    def patched_run_cargo_preflight(  # type: ignore[override]
        workspace_root,
        subcommand,
        *,
        runner,
        extra_args=None,
    ):
        return _patched_run_cargo_preflight(
            workspace_root,
            subcommand,
            runner=runner,
            extra_args=extra_args,
            publish_module=publish_module,
        )

    publish_module._run_cargo_preflight = patched_run_cargo_preflight  # type: ignore[assignment]


def _warm_cache_with_environment() -> None:
    """Pre-load cache entry when the workspace root is discoverable."""

    workspace_root = _resolve_workspace_root(os.environ.get(_PRE_FLIGHT_ENV))
    if workspace_root is not None:
        _load_preflight_test_excludes(workspace_root)


_patch_configuration_validation()
_patch_preflight_runner()
_warm_cache_with_environment()
