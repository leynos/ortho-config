"""Contract for the Windows-only cognitive-complexity gate.

Whitaker's lints run inside a compiler driver, so the suite scores only the
``cfg`` surface that compiler was asked to build. The host pass in
``make lint-whitaker`` therefore scores no ``#[cfg(windows)]`` item at all, and
the Windows continuous-integration leg runs Clippy alone: after PR #737 retired
Clippy's ``cognitive_complexity``, nothing scored Windows-only code (issue
#738).

``make lint-whitaker-windows`` closes that gap without a Windows Whitaker lane:
it runs the same suite at the Windows target from the Linux leg, so the lint,
the threshold and the macro-aware handling are the ones the rest of the
workspace already has. This contract holds both ends: the Makefile expresses
the pass, and continuous integration runs it.

The threshold is deliberately *not* restated here beyond the assertion that
both passes read the one ``dylint.toml``. A second copy in the workflow or the
recipe is how the two ends drift apart.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import re
import typing as typ

import pytest
from makefile_support import recipe_lines
from workflow_reading import read_workflows, workflow_jobs

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".github" / "workflows"
MAKEFILE_PATH: typ.Final[pathlib.Path] = REPOSITORY_ROOT / "Makefile"
DYLINT_CONFIGURATION: typ.Final[pathlib.Path] = REPOSITORY_ROOT / "dylint.toml"
PROBE_DIRECTORY: typ.Final[pathlib.Path] = (
    REPOSITORY_ROOT / "tests" / "fixtures" / "windows_complexity_probe"
)

#: The Make targets that make up the gate: the pass itself, the prerequisite
#: that provisions it, and the self-test that proves it can still fail.
LINT_TARGET: typ.Final[str] = "lint-whitaker-windows"
PREREQUISITE_TARGET: typ.Final[str] = "check-whitaker-windows-target"
INSTALL_TARGET: typ.Final[str] = "install-whitaker-windows-target"
SELF_TEST_TARGET: typ.Final[str] = "check-windows-complexity-gate"

#: The target triple the pass names, which is the one the Windows leg builds
#: natively. The Makefile's default and the self-test's default are held equal
#: to it instead of being restated as literals in three places.
WINDOWS_TARGET: typ.Final[str] = "x86_64-pc-windows-msvc"

#: The one lint this gate exists for, and the section that configures it. Both
#: passes read these, so the threshold and the macro handling cannot differ
#: between them.
COMPLEXITY_LINT: typ.Final[str] = "macro_aware_cognitive_complexity"
LINT_SECTION: typ.Final[str] = f"[{COMPLEXITY_LINT}]"
THRESHOLD_KEY: typ.Final[str] = "max_complexity"
THRESHOLD: typ.Final[str] = "9"

#: The assignment the pass must make, and the development flags it must not.
STRICT_FLAGS: typ.Final[str] = '-D warnings'
DEVELOPMENT_FLAGS: typ.Final[tuple[str, ...]] = (
    "-Zthreads=8",
    "-Zcodegen-backend=cranelift",
    "-Clink-arg=-fuse-ld=mold",
)

#: `WINDOWS_TARGET ?= <triple>`, read as the default the Makefile states.
WINDOWS_TARGET_SETTING: typ.Final[re.Pattern[str]] = re.compile(
    r"^WINDOWS_TARGET\s*\?=\s*(\S+)\s*$", re.MULTILINE
)


@pytest.fixture(scope="module")
def makefile() -> str:
    """Return the repository Makefile's text.

    Returns
    -------
    str
        The Makefile as written.
    """
    return MAKEFILE_PATH.read_text(encoding="utf-8")


@pytest.fixture(scope="module")
def jobs() -> dict[str, dict[str, object]]:
    """Return every job declared by any workflow, by name.

    Returns
    -------
    dict
        Job name to job mapping.
    """
    return {
        name: job
        for document in read_workflows(WORKFLOWS).values()
        for name, job in workflow_jobs(document).items()
    }


@pytest.fixture(scope="module")
def gate_recipe(makefile: str) -> list[str]:
    """Return the gate target's recipe lines.

    Parameters
    ----------
    makefile : str
        The repository Makefile's text.

    Returns
    -------
    list of str
        The recipe, empty when the target is absent.
    """
    return recipe_lines(makefile, LINT_TARGET)


def _run_steps(job: dict[str, object]) -> list[str]:
    """Return one job's `run:` bodies.

    Parameters
    ----------
    job : dict
        A parsed workflow job.

    Returns
    -------
    list of str
        Every shell body the job runs, in declaration order.
    """
    return [
        str(step["run"])
        for step in (job.get("steps") or [])
        if isinstance(step, dict) and "run" in step
    ]


def test_the_makefile_states_the_windows_target(makefile: str) -> None:
    """The triple is a variable, and it is the one the Windows leg builds.

    A literal repeated in the recipe would let the gate drift to another
    target without the self-test noticing, because the self-test takes the same
    variable.
    """
    (stated,) = WINDOWS_TARGET_SETTING.findall(makefile) or [None]
    assert stated == WINDOWS_TARGET, (
        f"WINDOWS_TARGET is {stated!r}, not the Windows leg's {WINDOWS_TARGET!r}"
    )


def test_the_gate_target_runs_whitaker_at_the_windows_target(
    gate_recipe: list[str],
) -> None:
    """The recipe is a real Whitaker pass at the Windows triple.

    ``--all`` is asserted because a scoped invocation that selects no library
    exits zero without checking anything, which is the failure mode that makes
    a clean run meaningless.
    """
    command = " ".join(gate_recipe)
    assert command, f"{LINT_TARGET} has no recipe"
    assert "$(WHITAKER) --all" in command, command
    assert "--target $(WINDOWS_TARGET)" in command, command
    assert "--all-targets" in command, command
    assert "--all-features" in command, command


def test_the_gate_target_takes_no_development_flags(gate_recipe: list[str]) -> None:
    """Whitaker's toolchain is not this repository's, so the standard is off.

    The pass restates `-D warnings` alone, exactly as the host pass does; the
    parallel frontend and `mold` belong to a compiler the suite does not run.
    """
    command = " ".join(gate_recipe)
    # `STRICT_RUSTFLAGS` is the variable both passes assign, and
    # `build_standard_routing_test.py` holds it to `-D warnings` alone. Naming
    # the variable here is what keeps the two routes on one definition.
    assert 'RUSTFLAGS="$(STRICT_RUSTFLAGS)"' in command, command
    assert "GATE_RUSTFLAGS" not in command, command
    for flag in DEVELOPMENT_FLAGS:
        assert flag not in command, f"{LINT_TARGET} carries {flag}"


def test_the_gate_depends_on_its_prerequisite(makefile: str) -> None:
    """Both the pass and the self-test check the standard library first.

    Without the prerequisite either one fails inside Cargo with a message
    about the target, which reads as a lint failure. Naming the check makes
    the missing component the thing that is reported.
    """
    for target in (LINT_TARGET, SELF_TEST_TARGET):
        rule = re.compile(
            rf"^{re.escape(target)}\s*:[^=\n]*\b{re.escape(PREREQUISITE_TARGET)}\b",
            re.MULTILINE,
        )
        assert rule.search(makefile), (
            f"{target} does not depend on {PREREQUISITE_TARGET}"
        )


def test_the_prerequisite_and_install_targets_share_the_variable(
    makefile: str,
) -> None:
    """Both provisioning targets pass the one triple variable.

    A literal here would provision a different target from the one the pass
    checks, and the pass would then fail for want of a component nobody
    installed.
    """
    for target in (PREREQUISITE_TARGET, INSTALL_TARGET):
        lines = recipe_lines(makefile, target)
        command = " ".join(lines)
        assert command, f"{target} has no recipe"
        assert "--target $(WINDOWS_TARGET)" in command, command


def test_the_threshold_and_macro_handling_are_shared_not_repeated(
    makefile: str, gate_recipe: list[str]
) -> None:
    """Both passes read one `dylint.toml`, and the recipe adds no override.

    The threshold and the experimental opt-in live in one file. A flag in the
    recipe, or a second configuration beside the probe, would be a second
    threshold to keep in step.
    """
    configuration = DYLINT_CONFIGURATION.read_text(encoding="utf-8")
    assert LINT_SECTION in configuration, configuration
    assert f"{THRESHOLD_KEY} = {THRESHOLD}" in configuration, configuration
    assert "[whitaker]" in configuration, "the suite is not told to load the lint"
    assert f'experimental = ["{COMPLEXITY_LINT}"]' in configuration, configuration
    command = " ".join(gate_recipe)
    assert THRESHOLD_KEY not in command, command
    assert "--max" not in command, command


def test_the_probe_states_the_same_threshold() -> None:
    """The self-test's crate is scored at the workspace's limit.

    A probe tuned to another limit would pass while the real gate failed, or
    the reverse, and either reads as evidence when it is not.
    """
    probe = (PROBE_DIRECTORY / "dylint.toml").read_text(encoding="utf-8")
    workspace = DYLINT_CONFIGURATION.read_text(encoding="utf-8")
    for setting in (f"{THRESHOLD_KEY} = {THRESHOLD}", f'["{COMPLEXITY_LINT}"]'):
        assert setting in probe, probe
    assert f"{THRESHOLD_KEY} = {THRESHOLD}" in workspace, workspace


def test_the_probe_carries_both_directions() -> None:
    """The fixture has an in-limit example and a feature-gated over-limit one.

    The feature is what lets one crate demonstrate a pass and a failure; an
    ungated over-limit example would fail every default build instead.
    """
    manifest = (PROBE_DIRECTORY / "Cargo.toml").read_text(encoding="utf-8")
    lib = (PROBE_DIRECTORY / "src" / "lib.rs").read_text(encoding="utf-8")
    assert "over_limit = []" in manifest, manifest
    assert "\n[workspace]\n" in manifest, (
        "the probe must stay out of the workspace, or --all-features fails it"
    )
    assert 'feature = "over_limit"' in lib, lib
    assert "mod in_limit;" in lib, lib


def test_the_self_test_target_runs_the_verifier(makefile: str) -> None:
    """The self-test target invokes the script, at the same triple and launcher.

    The script defaults to the same triple, so passing the variable is what
    keeps a `WINDOWS_TARGET=` override honest. `$(WHITAKER)` is passed for the
    same reason: a caller who overrides the launcher must not have the
    self-test validate whichever `whitaker` PATH resolves instead.
    """
    command = " ".join(recipe_lines(makefile, SELF_TEST_TARGET))
    assert command, f"{SELF_TEST_TARGET} has no recipe"
    assert "scripts/verify_windows_complexity_gate.py" in command, command
    assert "--target $(WINDOWS_TARGET)" in command, command
    assert "--whitaker" in command and "$(WHITAKER)" in command, command


def test_continuous_integration_runs_the_gate_and_its_self_test(
    jobs: dict[str, dict[str, object]],
) -> None:
    """A Linux job runs the pass and the self-test.

    Linux is the point: the suite is installed there, and the pass is what
    makes Windows-only code visible to it. A job on a Windows runner would be
    the Windows Whitaker lane the issue records as not required.
    """
    runners = {
        str(job.get("runs-on")): name
        for name, job in jobs.items()
        if f"make {LINT_TARGET}" in " ".join(_run_steps(job))
    }
    assert runners, f"no job runs `make {LINT_TARGET}`"
    (runner, job_name), = runners.items()
    assert "ubuntu" in runner or "linux" in runner, (
        f"{job_name} runs the Windows-target pass on {runner!r}, not Linux"
    )
    body = " ".join(_run_steps(jobs[job_name]))
    assert f"make {LINT_TARGET}" in body, body
    assert f"make {SELF_TEST_TARGET}" in body, body
    assert f"make {INSTALL_TARGET}" in body, (
        f"{job_name} never provisions the target's standard library"
    )


def test_the_host_pass_is_still_the_one_lint_runs(jobs: dict[str, dict[str, object]]) -> None:
    """Existing Linux coverage is untouched.

    The Linux leg keeps running `make lint`, which carries the host Whitaker
    pass; the Windows-target pass is added beside it, not in place of it.
    """
    lint_steps = [
        str(step.get("run"))
        for job in jobs.values()
        for step in (job.get("steps") or [])
        if isinstance(step, dict) and str(step.get("run", "")).strip() == "make lint"
    ]
    assert lint_steps, "no step runs `make lint`"
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    host_recipe = " ".join(recipe_lines(makefile, "lint-whitaker"))
    assert "$(WHITAKER) --all" in host_recipe, host_recipe
    assert "--target" not in host_recipe, (
        "the host pass grew a target argument; it must score the host surface"
    )


def test_the_gate_job_installs_whitaker_itself(
    jobs: dict[str, dict[str, object]],
) -> None:
    """The job that runs the pass also installs the suite.

    `whitaker_install_test.py` holds every install step to the pinned action
    on a Linux runner, and `sccache_wiring_test.py` holds the caching wiring.
    What neither can see is *which* job owns the install: a job that runs the
    pass without installing the suite borrows another job's, and would fail
    only when that job stopped running.
    """
    installing = [
        job_name
        for job_name, job in jobs.items()
        if f"make {LINT_TARGET}" in " ".join(_run_steps(job))
    ]
    assert installing, f"no job runs `make {LINT_TARGET}`"
    for job_name in installing:
        steps = jobs[job_name].get("steps") or []
        assert any(
            isinstance(step, dict) and step.get("name") == "Install Whitaker"
            for step in steps
        ), f"{job_name} runs `make {LINT_TARGET}` without installing Whitaker"