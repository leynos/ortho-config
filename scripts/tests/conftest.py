"""Shared fixtures for spelling rollout tests."""

from __future__ import annotations

import importlib
import types
from collections import abc as cabc
from pathlib import Path

import pytest

SCRIPT_DIRECTORY = Path(__file__).resolve().parents[1]
type _ModuleImporter = cabc.Callable[[tuple[str, ...]], tuple[types.ModuleType, ...]]


@pytest.fixture
def rollout_module_importer(
    monkeypatch: pytest.MonkeyPatch,
) -> _ModuleImporter:
    """Provide one import-path and cache-invalidation setup contract."""
    monkeypatch.syspath_prepend(str(SCRIPT_DIRECTORY))
    importlib.invalidate_caches()

    def import_modules(names: tuple[str, ...]) -> tuple[types.ModuleType, ...]:
        """Import the requested rollout helper modules."""
        return tuple(importlib.import_module(name) for name in names)

    return import_modules


@pytest.fixture(name="rollout_modules")
def rollout_modules_fixture(
    rollout_module_importer: _ModuleImporter,
) -> tuple[types.ModuleType, types.ModuleType, types.ModuleType]:
    """Import scripts through the top-level paths used at runtime."""
    names = ("typos_rollout_cache", "typos_rollout", "generate_typos_config")
    cache, rollout, generator = rollout_module_importer(names)
    return cache, rollout, generator
