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


@functools.lru_cache(maxsize=8)
def _load_preflight_test_excludes(workspace_root: Path | None) -> tuple[str, ...]:
    """Return crate names that pre-flight ``cargo test`` runs should skip."""

    if workspace_root is None:
        return ()
    config_path = workspace_root / "lading.toml"
    try:
        config_text = config_path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return ()
    try:
        parsed = tomllib.loads(config_text)
    except (tomllib.TOMLDecodeError, AttributeError):
        # Invalid configuration falls back to running the default suite. The
        # publish command will report a structured error when it attempts to
        # parse the file for its own options.
        return ()
    raw_preflight = parsed.get("preflight")
    if not isinstance(raw_preflight, dict):
        return ()
    raw_excludes = raw_preflight.get("test_exclude", ())
    if isinstance(raw_excludes, str):
        raw_items: Iterable[str] = (raw_excludes,)
    elif isinstance(raw_excludes, Iterable):
        raw_items = (
            item for item in raw_excludes if isinstance(item, str)
        )
    else:
        return ()
    excludes: list[str] = []
    for item in raw_items:
        candidate = item.strip()
        if candidate:
            excludes.append(candidate)
    return tuple(excludes)


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
            detail = (stderr or stdout).strip()
            message = (
                f"Pre-flight cargo {subcommand} failed with exit code {exit_code}"
            )
            if detail:
                message = f"{message}: {detail}"
            raise publish_module.PublishPreflightError(message)

    publish_module._run_cargo_preflight = patched_run_cargo_preflight  # type: ignore[assignment]


def _warm_cache_with_environment() -> None:
    """Pre-load cache entry when the workspace root is discoverable."""

    workspace_root = _resolve_workspace_root(os.environ.get(_PRE_FLIGHT_ENV))
    if workspace_root is not None:
        _load_preflight_test_excludes(workspace_root)


_patch_configuration_validation()
_patch_preflight_runner()
_warm_cache_with_environment()
