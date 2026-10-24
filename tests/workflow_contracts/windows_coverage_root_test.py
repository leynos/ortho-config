"""Contracts for the short Windows target root used by both coverage passes.

The measured `cargo-orthohelp` bin-test command reached 33,406 characters,
including 251 occurrences of the Cargo target root. Keep the Windows root
short enough to leave a deliberate margin below CreateProcess's limit while
leaving Linux's Cargo environment untouched.
"""

from __future__ import annotations

import pathlib
import typing as typ

from timeout_budgets import COVERAGE_ACTION
from workflow_reading import WorkflowDocument, read_workflows, workflow_jobs

WORKFLOWS: typ.Final[pathlib.Path] = (
    pathlib.Path(__file__).resolve().parents[2] / ".github" / "workflows"
)

EXPORT_STEP: typ.Final[str] = "Shorten the Cargo target root for coverage (Windows)"
RESTORE_STEP: typ.Final[str] = "Restore the default Cargo target root (Windows)"
WINDOWS_ONLY: typ.Final[str] = "${{ matrix.platform == 'windows' }}"
TARGET_ROOT_TEMPLATE: typ.Final[str] = r"$env:RUNNER_TEMP\cov"
WORKSPACE_TARGET_TEMPLATE: typ.Final[str] = r"$env:GITHUB_WORKSPACE\target"
LLVM_COV_TARGET_DIRECTORY: typ.Final[str] = "llvm-cov-target"
HOSTED_RUNNER_TEMP_PREFIX: typ.Final[str] = r"D:\a\_temp"

OBSERVED_COMMAND_LINE_LENGTH: typ.Final[int] = 33_406
CREATEPROCESS_COMMAND_LINE_LIMIT: typ.Final[int] = 32_767
TARGET_ROOT_OCCURRENCES: typ.Final[int] = 251
MEASURED_TARGET_ROOT_LENGTH: typ.Final[int] = 53
MINIMUM_HEADROOM: typ.Final[int] = 4_000
EXPECTED_PROJECTED_ROOT_LENGTH: typ.Final[int] = 30
EXPECTED_PROJECTED_COMMAND_LINE_LENGTH: typ.Final[int] = 27_633
EXPECTED_PROJECTED_HEADROOM: typ.Final[int] = 5_134
LONGER_CARGO_TARGET_HEADROOM: typ.Final[int] = 2_875

LINUX_UNSET_EXPLANATION: typ.Final[str] = (
    "Linux cells must stay genuinely unset; an empty value is not 'unset'."
)
TARGET_ROOT_ENVIRONMENT_VARIABLES: typ.Final[frozenset[str]] = frozenset(
    {"CARGO_TARGET_DIR", "CARGO_LLVM_COV_TARGET_DIR"}
)
ALLOWED_TARGET_ROOT_RUNS: typ.Final[frozenset[tuple[str, str, str]]] = frozenset(
    {
        ("build-test", EXPORT_STEP, "CARGO_TARGET_DIR"),
        ("build-test", RESTORE_STEP, "CARGO_TARGET_DIR"),
    }
)


def _ci_workflow() -> WorkflowDocument:
    """Read `ci.yml` using the workflow contract's shared YAML reader."""
    return read_workflows(WORKFLOWS)["ci.yml"]


def _build_test_steps(ci: WorkflowDocument) -> list[dict[str, object]]:
    """Return `build-test` steps, refusing malformed YAML structure."""
    build_test = workflow_jobs(ci)["build-test"]
    steps = build_test.get("steps")
    assert isinstance(steps, list), "build-test steps must be a list"
    assert all(isinstance(step, dict) for step in steps), (
        "each build-test step must be a mapping"
    )
    return steps


def _named_step(
    steps: list[dict[str, object]], name: str
) -> tuple[int, dict[str, object]]:
    """Return the unique step with `name`, or fail with its observed count."""
    matching = [
        (index, step) for index, step in enumerate(steps) if step.get("name") == name
    ]
    assert len(matching) == 1, f"found {len(matching)} build-test steps named {name!r}"
    return matching[0]


def _normalised_run(step: dict[str, object]) -> str:
    """Collapse YAML block whitespace to compare the command as one line."""
    return " ".join(str(step.get("run", "")).split())


def _step_condition(step: dict[str, object]) -> str:
    """Return the step condition with YAML whitespace normalised."""
    return " ".join(str(step.get("if", "")).split())


def _environment_blocks(ci: WorkflowDocument) -> list[tuple[str, object]]:
    """Return workflow-, job-, and step-level environment declarations."""
    blocks = [("workflow", ci.get("env"))]
    for job_name, job in workflow_jobs(ci).items():
        blocks.append((f"job {job_name!r}", job.get("env")))
        steps = job.get("steps", [])
        if isinstance(steps, list):
            blocks.extend(
                (f"step {job_name!r}[{index}]", step.get("env"))
                for index, step in enumerate(steps)
                if isinstance(step, dict)
            )
    return blocks


def _projected_command_line_length(target_root_length: int) -> int:
    r"""Project the measured command after shortening every root occurrence.

    >>> _projected_command_line_length(30)
    27633
    """
    return OBSERVED_COMMAND_LINE_LENGTH - TARGET_ROOT_OCCURRENCES * (
        MEASURED_TARGET_ROOT_LENGTH - target_root_length
    )


def test_windows_coverage_root_steps_keep_their_guard_shell_and_exact_writes() -> None:
    """The two Windows steps write the target roots through UTF-8 GITHUB_ENV."""
    steps = _build_test_steps(_ci_workflow())
    _, export = _named_step(steps, EXPORT_STEP)
    _, restore = _named_step(steps, RESTORE_STEP)

    for name, step in ((EXPORT_STEP, export), (RESTORE_STEP, restore)):
        assert _step_condition(step) == WINDOWS_ONLY, (
            f"{name!r} must run only on Windows"
        )
        assert step.get("shell") == "pwsh", f"{name!r} must use PowerShell 7"

    assert _normalised_run(export) == (
        "Add-Content -Path $env:GITHUB_ENV -Value "
        f'"CARGO_TARGET_DIR={TARGET_ROOT_TEMPLATE}" -Encoding utf8'
    ), "the export must append exactly one UTF-8, BOM-free target-root line"
    assert _normalised_run(restore) == (
        "Add-Content -Path $env:GITHUB_ENV -Value "
        f'"CARGO_TARGET_DIR={WORKSPACE_TARGET_TEMPLATE}" -Encoding utf8'
    ), "the restore must append exactly one UTF-8, BOM-free target-root line"


def test_windows_coverage_root_steps_bracket_both_coverage_passes() -> None:
    """The same root covers both passes and is restored before later consumers."""
    steps = _build_test_steps(_ci_workflow())
    coverage_indices = [
        index
        for index, step in enumerate(steps)
        if COVERAGE_ACTION in str(step.get("uses", ""))
    ]
    assert len(coverage_indices) == 2, (
        f"build-test has {len(coverage_indices)} coverage actions, not two"
    )
    first_index, second_index = coverage_indices
    export_index, _ = _named_step(steps, EXPORT_STEP)
    restore_index, _ = _named_step(steps, RESTORE_STEP)
    lint_index, _ = _named_step(steps, "Lint")
    wrapper_index, _ = _named_step(steps, "Validate PowerShell wrapper")
    publish_index, _ = _named_step(steps, "Publish dry run")

    assert lint_index < export_index < first_index, (
        "the Windows root export must follow lint and precede coverage"
    )
    assert first_index < second_index < restore_index, (
        "one target root must cover both passes before the restore"
    )
    assert wrapper_index > restore_index and publish_index > restore_index, (
        "the default root must be restored before wrapper validation and publish"
    )
    assert not any(
        "CARGO_TARGET_DIR" in _normalised_run(step)
        for step in steps[first_index + 1 : second_index]
    ), (
        "CARGO_TARGET_DIR must not change between coverage passes; "
        f"{LINUX_UNSET_EXPLANATION}"
    )
    for action_index in coverage_indices:
        action = steps[action_index]
        assert not {"env", "if", "continue-on-error"}.intersection(action), (
            "coverage actions must not override the Windows-only target-root scope"
        )


def test_ci_has_no_target_root_environment_overrides() -> None:
    """Keep Linux target roots genuinely unset at every workflow scope."""
    ci = _ci_workflow()
    declared = [
        (scope, name)
        for scope, block in _environment_blocks(ci)
        if isinstance(block, dict)
        for name in TARGET_ROOT_ENVIRONMENT_VARIABLES.intersection(block)
    ]
    assert not declared, (
        f"ci.yml declares target-root overrides in env blocks: {declared}; "
        f"{LINUX_UNSET_EXPLANATION}"
    )


def _target_root_run_mentions(step: object) -> list[tuple[str, str]]:
    r"""List target-root declarations in one run block.

    This parser belongs to the Windows coverage workflow contract; keep it
    local to this test rather than treating it as a general workflow reader.

    >>> _target_root_run_mentions({"name": "Build", "run": "echo CARGO_TARGET_DIR=target"})
    [('Build', 'CARGO_TARGET_DIR')]
    >>> _target_root_run_mentions({"name": "Build", "run": "echo CARGO_LLVM_COV_TARGET_DIR=target"})
    [('Build', 'CARGO_LLVM_COV_TARGET_DIR')]
    """
    if not isinstance(step, dict):
        return []
    run_block = str(step.get("run", ""))
    step_name = str(step.get("name", ""))
    return [
        (step_name, variable)
        for variable in sorted(TARGET_ROOT_ENVIRONMENT_VARIABLES)
        if variable in run_block
    ]


def _unexpected_target_root_mentions_for_job(
    job_name: str, job: dict[str, object]
) -> list[tuple[str, int, str, str]]:
    """Return run-block mentions outside the two allowed build-test steps."""
    steps = job.get("steps", [])
    if not isinstance(steps, list):
        return []

    unexpected_mentions = []
    for index, step in enumerate(steps):
        for step_name, variable in _target_root_run_mentions(step):
            if (job_name, step_name, variable) not in ALLOWED_TARGET_ROOT_RUNS:
                unexpected_mentions.append(
                    (job_name, index, step_name, variable)
                )
    return unexpected_mentions


def test_ci_has_no_unexpected_target_root_run_mentions() -> None:
    """Only the two build-test steps may mention target-root variables."""
    ci = _ci_workflow()
    unexpected_mentions = [
        mention
        for job_name, job in workflow_jobs(ci).items()
        for mention in _unexpected_target_root_mentions_for_job(job_name, job)
    ]
    assert not unexpected_mentions, (
        "CARGO_TARGET_DIR may appear only in the export and restore steps, and "
        "CARGO_LLVM_COV_TARGET_DIR must not appear in run blocks; "
        f"found {unexpected_mentions}. {LINUX_UNSET_EXPLANATION}"
    )


def test_short_windows_root_keeps_the_createprocess_headroom() -> None:
    """The hosted root stays short enough for a growing dependency set."""
    hosted_runner_temp = pathlib.PureWindowsPath(HOSTED_RUNNER_TEMP_PREFIX)
    target_root_component = pathlib.PureWindowsPath(TARGET_ROOT_TEMPLATE).name
    hosted_target_root = (
        hosted_runner_temp / target_root_component / LLVM_COV_TARGET_DIRECTORY
    )
    projected_length = _projected_command_line_length(len(str(hosted_target_root)))
    headroom = CREATEPROCESS_COMMAND_LINE_LIMIT - projected_length

    assert len(str(hosted_target_root)) == EXPECTED_PROJECTED_ROOT_LENGTH, (
        f"hosted coverage root {hosted_target_root!r} has length "
        f"{len(str(hosted_target_root))}, not {EXPECTED_PROJECTED_ROOT_LENGTH}"
    )
    assert projected_length == EXPECTED_PROJECTED_COMMAND_LINE_LENGTH, (
        f"projected command length is {projected_length}, not "
        f"{EXPECTED_PROJECTED_COMMAND_LINE_LENGTH}"
    )
    assert headroom == EXPECTED_PROJECTED_HEADROOM, (
        f"projected CreateProcess headroom is {headroom}, not "
        f"{EXPECTED_PROJECTED_HEADROOM}"
    )
    assert headroom >= MINIMUM_HEADROOM, (
        f"projected CreateProcess headroom {headroom} is below "
        f"{MINIMUM_HEADROOM} characters"
    )

    longer_root_component = pathlib.PureWindowsPath(TARGET_ROOT_TEMPLATE).with_name(
        "cargo-target"
    )
    longer_root = (
        hosted_runner_temp / longer_root_component.name / LLVM_COV_TARGET_DIRECTORY
    )
    longer_headroom = CREATEPROCESS_COMMAND_LINE_LIMIT - (
        _projected_command_line_length(len(str(longer_root)))
    )
    assert len(str(longer_root)) == 39, (
        f"cargo-target root shape changed: {longer_root!r}"
    )
    assert longer_headroom == LONGER_CARGO_TARGET_HEADROOM, (
        f"cargo-target root headroom is {longer_headroom}, not "
        f"{LONGER_CARGO_TARGET_HEADROOM}"
    )
    assert longer_headroom < MINIMUM_HEADROOM, (
        f"cargo-target leaves {longer_headroom} characters, below the required "
        f"{MINIMUM_HEADROOM}"
    )
