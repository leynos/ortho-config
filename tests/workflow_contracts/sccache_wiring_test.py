"""Contract tests for the sccache wiring across the Rust workflows.

sccache needs two halves to do anything: a rustc wrapper, so the
compiler is actually invoked through it, and a backend, so the cache has
somewhere to live. Either half alone is worse than neither, because the
job pays the install and reports nothing amiss.

This repository ran with neither for a long time. The shared Rust setup
action installs sccache whenever ``use-sccache`` is true, which is its
default, and at pin ``32c8ea64`` it exported neither ``RUSTC_WRAPPER``
nor ``SCCACHE_GHA_ENABLED``. Every Rust job here installed sccache and
compiled uncached.

The wrapper half is deliberately not set here. A bare
``RUSTC_WRAPPER: sccache`` resolves through ``PATH``, and an invocation
that resolves nothing compiles uncached without entering the hit-rate
denominator, so the failure is invisible in the statistic a reader would
check; whitaker #409 measured 69 such invocations. The action exports
the absolute path of the sccache it installed, and stands aside when a
caller has already set the variable, so a value here would replace that
path with a bare name.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

import pytest
import yaml

WORKFLOWS: typ.Final[Path] = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows"
)

#: The action whose presence marks a job as one the shared Rust setup
#: prepares, and therefore one where sccache is installed.
SETUP_RUST: typ.Final[str] = "leynos/shared-actions/.github/actions/setup-rust@"

#: Every reference into the shared-actions tree must move as one. A
#: partial repin leaves the repository unable to say which SHA it is on,
#: and the half left behind is the half that silently keeps the old
#: behaviour.
#:
#: Matched by repository prefix rather than by an enumerated list of
#: paths. A list is closed: a governed reference added later under a
#: path nobody thought to add would sit at any SHA it liked and this
#: contract would report agreement.
SHARED_ACTIONS_PREFIX: typ.Final[str] = "leynos/shared-actions/"

#: The references deliberately left at their own pin, each with the
#: reason. Named individually rather than allowed as a class, so adding
#: a third is a decision somebody has to write down.
#:
#: Keyed by the whole path inside the shared-actions tree and matched by
#: equality. A substring match would exempt every sibling whose path
#: merely begins with one of these: a later
#: ``.github/actions/rust-build-release-extra`` is a different action at
#: a pin of its own, and skipping it would drop its SHA from the set the
#: one-SHA assertion reads, so a partial repin would report agreement.
PINNED_SEPARATELY: typ.Final[dict[str, str]] = {
    ".github/actions/rust-build-release": (
        "serves the release build, which the action excludes from sccache"
    ),
    ".github/workflows/mutation-cargo.yml": (
        "a reusable workflow, which caller job env cannot reach"
    ),
}

#: Pins known to install sccache while exporting neither half. Reverting
#: to one of these is the regression this contract exists to catch, and
#: naming them is cheaper and more honest than fetching the action's text
#: over the network from a test that must run offline.
PINS_WITHOUT_THE_WRAPPER: typ.Final[frozenset[str]] = frozenset(
    {
        "32c8ea649ea44d40119f348ad48861212532061f",
    }
)

#: Jobs that run the shared Rust setup and are nevertheless allowed no
#: backend, each with the reason it compiles nothing worth caching.
#:
#: `verify-published-assets` is not excluded because of the event. This
#: workflow triggers on a tag push and on workflow_dispatch, never on
#: `release`, so the action's own `github.event_name != 'release'` guard
#: does not reach it. It is excluded because it dry-runs `cargo binstall`
#: against already published archives and builds nothing.
NO_BACKEND_EXPECTED: typ.Final[dict[str, str]] = {
    "verify-published-assets": "dry-runs cargo binstall against published archives",
}

_SHA = re.compile(r"@([0-9a-f]{40})\b")

#: A same-tree reference, split into the path inside the shared-actions
#: tree and whatever follows the ``@``. The reference is read as a whole
#: rather than scanned for a SHA, so the path can be compared exactly and
#: an unpinned ``@v1`` still reaches the assertion below.
_REFERENCE = re.compile(
    re.escape("leynos/shared-actions/") + r"(?P<path>[^@\s'\"]+)@(?P<ref>[^\s'\"]+)"
)


def _workflow_text() -> dict[str, str]:
    """Read every workflow document as text.

    Returns
    -------
    dict[str, str]
        File name to raw text. The single reader; the parsed view below
        is derived from it so the two cannot disagree about which files
        are in scope.
    """
    # Both suffixes. GitHub runs a workflow named either way, so a sweep
    # over one of them reports repository-wide coverage while ignoring
    # half the places a Rust job can be declared.
    return {
        path.name: path.read_text(encoding="utf-8")
        for pattern in ("*.yml", "*.yaml")
        for path in sorted(WORKFLOWS.glob(pattern))
    }


def _workflow_documents() -> dict[str, dict[str, typ.Any]]:
    """Parse every workflow document.

    Returns
    -------
    dict[str, dict]
        File name to parsed document.
    """
    return {name: yaml.safe_load(text) for name, text in _workflow_text().items()}


def _pinned_shas() -> dict[str, set[str]]:
    """Map each SHA a same-tree reference names to the files naming it.

    Returns
    -------
    dict[str, set[str]]
        Commit SHA to the workflow file names that pin it.

    Raises
    ------
    AssertionError
        If a reference is not pinned to a 40-hex commit SHA.
    """
    found: dict[str, set[str]] = {}
    for name, text in _workflow_text().items():
        for line in text.splitlines():
            if SHARED_ACTIONS_PREFIX not in line:
                continue
            reference = _REFERENCE.search(line)
            assert reference is not None, (
                f"{name}: a {SHARED_ACTIONS_PREFIX} reference could not be "
                f"read as a path and a ref: {line.strip()}"
            )
            if reference.group("path") in PINNED_SEPARATELY:
                continue
            assert _SHA.fullmatch("@" + reference.group("ref")) is not None, (
                f"{name}: a {SHARED_ACTIONS_PREFIX} reference is not pinned "
                f"to a 40-hex commit SHA: {line.strip()}"
            )
            found.setdefault(reference.group("ref"), set()).add(name)
    return found


def _jobs_running_setup_rust() -> dict[str, dict[str, typ.Any]]:
    """Find every job with a shared Rust setup step.

    Returns
    -------
    dict[str, dict]
        Job name to the job's mapping. Discovered from the documents
        rather than listed, because a new Rust job appearing without a
        backend is the drift this guards.
    """
    found: dict[str, dict[str, typ.Any]] = {}
    for document in _workflow_documents().values():
        for name, job in (document.get("jobs") or {}).items():
            steps = job.get("steps") or []
            if any(str(step.get("uses", "")).startswith(SETUP_RUST) for step in steps):
                found[name] = job
    return found


def test_the_shared_action_references_share_one_sha() -> None:
    """Assert every same-tree reference sits at a single commit.

    A partial repin is the failure: the guide and the pull request body
    both claim one SHA, and a single reference left behind makes that
    claim false while looking like housekeeping in a diff.
    """
    found = _pinned_shas()
    assert found, "no shared-actions references were found to check"
    assert len(found) == 1, (
        "the shared-actions references are split across more than one SHA: "
        f"{ {sha: sorted(files) for sha, files in found.items()} }"
    )


def test_the_pin_is_not_one_that_exports_no_wrapper() -> None:
    """Assert the pin is not a revision that leaves sccache unwired.

    `32c8ea64` installs sccache and exports neither the wrapper nor a
    backend. Returning to it would restore a state in which every Rust
    job pays the install and compiles uncached, which no other assertion
    here would notice.
    """
    text = "\n".join(_workflow_text().values())
    offending = sorted(pin for pin in PINS_WITHOUT_THE_WRAPPER if pin in text)
    assert not offending, (
        f"these pins install sccache without exporting the rustc wrapper: {offending}"
    )


def test_no_workflow_sets_the_rustc_wrapper_by_name() -> None:
    """Assert the wrapper comes from the action, never from a workflow.

    A bare name resolves through `PATH`, and an invocation that resolves
    nothing compiles uncached without entering the hit-rate denominator.
    The action stands aside when the caller has set the variable, so a
    value here silently wins.
    """
    setters = sorted(
        name
        for name, text in _workflow_text().items()
        if re.search(r"^\s*RUSTC_WRAPPER\s*:", text, re.MULTILINE)
    )
    assert not setters, (
        "these workflows set RUSTC_WRAPPER, which overrides the absolute path "
        f"the action exports: {setters}"
    )


def test_the_sweep_finds_the_jobs_that_run_the_rust_setup() -> None:
    """Assert the discovery is not empty and names a job it must find.

    The two assertions below are both satisfied by a sweep that returns
    nothing, so the sweep itself is pinned.
    """
    jobs = _jobs_running_setup_rust()
    assert jobs, "no job was found running the shared Rust setup"
    assert "build-test" in jobs, "the sweep missed the main CI job"


@pytest.mark.parametrize("job_name", sorted(_jobs_running_setup_rust()))
def test_every_rust_job_selects_a_backend(job_name: str) -> None:
    """Assert each job running the Rust setup selects the cache backend.

    Without a backend sccache falls back to a local directory the runner
    discards, so the wrapper the action exports caches into nothing. The
    exclusions are named individually with the reason they compile
    nothing, rather than being a blanket allowance.
    """
    job = _jobs_running_setup_rust()[job_name]
    environment = job.get("env") or {}
    if job_name in NO_BACKEND_EXPECTED:
        assert "SCCACHE_GHA_ENABLED" not in environment, (
            f"{job_name} is recorded as needing no backend "
            f"({NO_BACKEND_EXPECTED[job_name]}) but sets one"
        )
        return
    assert environment.get("SCCACHE_GHA_ENABLED") == "true", (
        f"{job_name} runs the shared Rust setup, so it installs sccache, but "
        "does not set SCCACHE_GHA_ENABLED and would cache into a discarded "
        "local directory"
    )


@pytest.mark.parametrize("job_name", sorted(_jobs_running_setup_rust()))
def test_every_job_with_a_backend_reports_its_statistics(job_name: str) -> None:
    """Assert each caching job prints sccache's own statistics.

    The wiring is invisible from the outside: a job with no wrapper and a
    job with one both succeed, and only the compile-request and hit
    counts tell them apart. Without this step there is no way to prove
    the cache is being used, or to notice later that it stopped.

    The command is asserted, not the step name, and it must invoke the
    binary through `SCCACHE_PATH`. A bare `sccache --show-stats` reports
    on whichever binary `PATH` resolves, which is the same defect the
    wrapper half of this contract exists to prevent.

    The step's condition is asserted too. A step with no condition, or
    one guarded on success, reports nothing when the build fails, and a
    failed build is exactly the run whose compile-request and hit counts
    explain it. Reading only the joined commands cannot see that: the
    text is identical either way.
    """
    job = _jobs_running_setup_rust()[job_name]
    if job_name in NO_BACKEND_EXPECTED:
        pytest.skip(f"{job_name} caches nothing: {NO_BACKEND_EXPECTED[job_name]}")
    steps = job.get("steps") or []
    reporting = [step for step in steps if "--show-stats" in str(step.get("run", ""))]
    assert reporting, (
        f"{job_name} selects an sccache backend but never reports its "
        "statistics, so the wiring cannot be verified from a run"
    )
    commands = "\n".join(str(step.get("run", "")) for step in reporting)
    assert "SCCACHE_PATH" in commands, (
        f"{job_name} reports sccache statistics without invoking it through "
        "SCCACHE_PATH, so it may report on a different binary"
    )
    unconditional = [
        str(step.get("name", "<unnamed>"))
        for step in reporting
        if str(step.get("if", "")).strip() != "always()"
    ]
    assert not unconditional, (
        f"{job_name} reports sccache statistics from a step that does not run "
        f"unconditionally, so a failed build reports nothing: {unconditional}"
    )
