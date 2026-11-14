"""Contracts for Cargo's per-host development-standard configuration.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from build_standard_make import (
    BACKEND_FLAG,
    CHECK_CFG_FLAG,
    DEV_FAST_CFG_FLAG,
    HOSTS,
    LINKER_FLAG,
    THREADS_FLAG,
    configured_flags,
    load_cargo_configuration,
)

CARGO_CONFIGURATION = load_cargo_configuration()


def test_the_standard_is_the_whole_standard_on_each_platform() -> None:
    """Pin each platform's parsed config to its exact standard flags."""
    # Cranelift is disabled by default (14 November 2026), so the backend and
    # the dev-fast cfg are opt-ins and the standard is the frontend and the
    # registered cfg name.
    common = {THREADS_FLAG, CHECK_CFG_FLAG}
    assert not {BACKEND_FLAG, DEV_FAST_CFG_FLAG} & common
    for host_name in ("linux_x86_64", "linux_aarch64"):
        flags = configured_flags(HOSTS[host_name], CARGO_CONFIGURATION)
        assert flags == common | {LINKER_FLAG}, host_name
    for host_name in ("macos_arm64", "windows_x86_64"):
        flags = configured_flags(HOSTS[host_name], CARGO_CONFIGURATION)
        assert flags == common, host_name
