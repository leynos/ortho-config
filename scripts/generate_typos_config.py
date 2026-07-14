#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = []
# ///
"""Generate ``typos.toml`` from the shared en-GB-oxendict dictionary.

The shared dictionary is refreshed into an untracked repository-local cache
only when the authoritative copy is newer. A valid cache remains usable when
the network is unavailable, and ``typos.local.toml`` supplies the narrow
repository-specific policy that must not weaken the estate-wide base.
"""

from pathlib import Path

import typos_rollout as rollout

DEFAULT_BASE_URL = (
    "https://raw.githubusercontent.com/leynos/agent-helper-scripts/"
    "refs/heads/main/data/typos-oxendict-base.toml"
)
REPOSITORY_ROOT = Path(__file__).resolve().parent.parent


def dictionary_from_cache(repository: Path = REPOSITORY_ROOT) -> rollout.Dictionary:
    """Load the cached shared base merged with local repository policy.

    Parameters
    ----------
    repository
        Repository containing the base cache and optional local overlay.

    Returns
    -------
    rollout.Dictionary
        Validated merged spelling policy.

    Raises
    ------
    OSError, UnicodeDecodeError, TypeError, ValueError
        If policy files cannot be read or fail validation.
    """
    dictionary = rollout.load_dictionary(repository / ".typos-oxendict-base.toml")
    local_overlay = repository / "typos.local.toml"
    if local_overlay.exists():
        dictionary = rollout.merge_dictionaries(
            dictionary,
            rollout.load_dictionary(local_overlay, local_overlay=True),
        )
    return dictionary


def render_config(repository: Path = REPOSITORY_ROOT) -> str:
    """Render deterministic configuration from the populated local cache.

    Parameters
    ----------
    repository
        Repository containing cached spelling policy.

    Returns
    -------
    str
        Generated ``typos.toml`` document.

    Raises
    ------
    OSError, UnicodeDecodeError, TypeError, ValueError
        If policy cannot be read, merged or rendered.
    """
    return rollout.render_typos_config(dictionary_from_cache(repository))


def main(
    output: Path | None = None,
    *,
    repository: Path = REPOSITORY_ROOT,
    source: str | Path = DEFAULT_BASE_URL,
    offline: bool = False,
) -> rollout.RefreshResult:
    """Refresh the shared base cache and write merged configuration.

    Parameters
    ----------
    output
        Destination path, defaulting to the repository's ``typos.toml``.
    repository
        Repository containing cache and local policy files.
    source
        Local path or HTTPS URL for the shared spelling authority.
    offline
        Whether to require an existing valid cache without network access.

    Returns
    -------
    rollout.RefreshResult
        Refresh status and validated cache path.

    Raises
    ------
    OSError, UnicodeDecodeError, TypeError, ValueError
        If refresh, validation, rendering or persistence fails.
    """
    destination = output if output is not None else repository / "typos.toml"
    result = rollout.refresh_base(
        source,
        repository / ".typos-oxendict-base.toml",
        rollout.RefreshOptions(
            metadata=repository / ".typos-oxendict-base.json",
            offline=offline,
        ),
    )
    rollout.write_config(destination, dictionary_from_cache(repository))
    return result


if __name__ == "__main__":
    refresh = main()
    print(f"{refresh.status}: {REPOSITORY_ROOT / 'typos.toml'}")
