"""Contract for the pinned merger and its exact coverage inputs.

These tests pin both workflow commands and exercise their guards against
missing, empty and unexpected reports.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import os
import pathlib
import re
import shutil
import subprocess
import typing as typ

import pytest

from workflow_reading import load_workflow, workflow_jobs

#: The repository's workflow directory.
WORKFLOW_DIR: typ.Final[pathlib.Path] = (
    pathlib.Path(__file__).resolve().parents[2] / ".github" / "workflows"
)

#: The workflow jobs whose merge scripts share the coverage contract.
WORKFLOW_CASES: typ.Final[tuple[tuple[str, str], ...]] = (
    ("ci.yml", "build-test"),
    ("coverage-main.yml", "coverage-upload"),
)

#: Readable pytest IDs in the same order as the workflow cases.
WORKFLOW_IDS: typ.Final[tuple[str, ...]] = (
    "ci-build-test",
    "coverage-main-upload",
)

#: The only reports that the merger accepts.
ALLOWED_REPORTS: typ.Final[tuple[str, ...]] = (
    "lcov-serde-saphyr.info",
    "lcov-no-serde-saphyr.info",
)

#: The exact file pattern passed as the merger's sole argument.
MERGER_INPUT_PATTERN: typ.Final[str] = (
    "{lcov-serde-saphyr.info,lcov-no-serde-saphyr.info}"
)

#: The reviewed package version required in both workflows.
MERGER_VERSION: typ.Final[str] = "6.0.0"

#: A small report the stub emits unchanged when the merge succeeds.
LCOV_OUTPUT: typ.Final[str] = "TN:\nSF:src/lib.rs\nDA:1,1\nend_of_record\n"


def _job(workflow_name: str, job_name: str) -> dict[str, object]:
    """Load one named job from a workflow."""
    workflow_path = WORKFLOW_DIR / workflow_name
    document = load_workflow(workflow_path.read_text(encoding="utf-8"))
    return workflow_jobs(document)[job_name]


def _steps(job: dict[str, object]) -> list[dict[str, object]]:
    """Return the mapping steps declared by a workflow job."""
    raw_steps = job.get("steps")
    assert isinstance(raw_steps, list), f"expected a list of steps; got {raw_steps!r}"
    assert all(isinstance(step, dict) for step in raw_steps), (
        f"expected every step to be a mapping; got {raw_steps!r}"
    )
    return [step for step in raw_steps if isinstance(step, dict)]


def _merge_step(steps: list[dict[str, object]]) -> dict[str, object]:
    """Return the single named merge step from a job."""
    merge_steps = [
        step for step in steps if step.get("name") == "Merge coverage results"
    ]
    assert len(merge_steps) == 1, (
        f"expected exactly one merge step; got {len(merge_steps)}"
    )
    return merge_steps[0]


def _merge_script(workflow_name: str, job_name: str) -> str:
    """Return the merge script extracted from a workflow job."""
    steps = _steps(_job(workflow_name, job_name))
    return str(_merge_step(steps).get("run", ""))


def _merger_command(script: str) -> str:
    """Return the merger command line from an extracted script."""
    commands = [
        line.strip()
        for line in script.splitlines()
        if line.strip().startswith("bun x lcov-result-merger")
    ]
    assert len(commands) == 1, (
        f"expected exactly one merger command; got {commands!r}"
    )
    return commands[0]


def _pinned_version(script: str) -> str:
    """Return the exact semantic version used by the merger command."""
    command = _merger_command(script)
    match = re.fullmatch(
        r"bun x lcov-result-merger@(?P<version>\d+\.\d+\.\d+) "
        r"'(?P<inputs>[^']+)' > lcov\.info",
        command,
    )
    assert match is not None, (
        "expected a merger invocation with an exact X.Y.Z pin and one input "
        f"pattern; got {command!r}"
    )
    return match.group("version")


def _run_script(
    script: str,
    working_directory: pathlib.Path,
    bash_executable: str,
    bun_output: str,
) -> tuple[subprocess.CompletedProcess[str], pathlib.Path]:
    """Run a merge script with a local `bun` stub and return its result."""
    stub_directory = working_directory / "bin"
    stub_directory.mkdir()
    argument_path = working_directory / "bun-arguments.txt"
    bun_stub = stub_directory / "bun"
    bun_stub.write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        "printf '%s\\n' \"$@\" > \"$BUN_ARGUMENTS_FILE\"\n"
        "printf '%s' \"$BUN_OUTPUT\"\n",
        encoding="utf-8",
    )
    bun_stub.chmod(0o755)
    environment = {
        "PATH": f"{stub_directory}{os.pathsep}{os.defpath}",
        "BUN_ARGUMENTS_FILE": str(argument_path),
        "BUN_OUTPUT": bun_output,
    }
    result = subprocess.run(
        [bash_executable],
        input=script,
        cwd=working_directory,
        env=environment,
        capture_output=True,
        check=False,
        text=True,
    )
    return result, argument_path


def _prepare_reports(
    working_directory: pathlib.Path,
    invalid_report: tuple[str, str] | None = None,
) -> None:
    """Create valid reports, then apply one requested invalid-input case."""
    for report in ALLOWED_REPORTS:
        (working_directory / report).write_text("TN:\n", encoding="utf-8")

    match invalid_report:
        case None:
            return
        case ("missing", report):
            (working_directory / report).unlink()
        case ("empty", report):
            (working_directory / report).write_text("", encoding="utf-8")
        case ("extra", report):
            (working_directory / report).write_text("TN:\n", encoding="utf-8")
        case _:
            raise ValueError(f"unsupported invalid report case: {invalid_report!r}")


@pytest.mark.parametrize(
    ("workflow_name", "job_name"), WORKFLOW_CASES, ids=WORKFLOW_IDS
)
def test_each_workflow_has_one_exactly_pinned_merge_step(
    workflow_name: str, job_name: str
) -> None:
    """Each owner runs one merger with an exact version and report pattern."""
    steps = _steps(_job(workflow_name, job_name))
    merge_step = _merge_step(steps)
    script = str(merge_step.get("run", ""))
    command = _merger_command(script)

    assert merge_step.get("shell") == "bash", (
        f"expected the merge shell to be 'bash'; got {merge_step.get('shell')!r}"
    )
    assert script.startswith("set -euo pipefail"), (
        f"expected strict mode at the script start; got {script.splitlines()[:1]!r}"
    )
    assert 'if ! test -s "$report"; then' in script, (
        f"expected the input guard to use test -s; got {script!r}"
    )
    assert "if ! test -s lcov.info; then" in script, (
        f"expected the output guard to use test -s; got {script!r}"
    )
    assert _pinned_version(script) == MERGER_VERSION, (
        f"expected merger version {MERGER_VERSION!r}; "
        f"got {_pinned_version(script)!r}"
    )
    assert command == (
        f"bun x lcov-result-merger@{MERGER_VERSION} "
        f"'{MERGER_INPUT_PATTERN}' > lcov.info"
    ), f"expected the exact merger command; got {command!r}"


def test_both_workflows_share_identical_pinned_merge_scripts() -> None:
    """Both workflows use the same exact merger version and guard."""
    first_script, second_script = (
        _merge_script(workflow_name, job_name)
        for workflow_name, job_name in WORKFLOW_CASES
    )
    first_version = _pinned_version(first_script)
    second_version = _pinned_version(second_script)
    assert first_version == second_version, (
        f"expected matching merger versions; got {first_version!r} and "
        f"{second_version!r}"
    )
    assert first_script == second_script, (
        f"expected identical merge scripts; got {first_script!r} and "
        f"{second_script!r}"
    )


@pytest.mark.parametrize(
    ("workflow_name", "job_name"), WORKFLOW_CASES, ids=WORKFLOW_IDS
)
def test_merge_follows_both_generation_steps_and_precedes_upload(
    workflow_name: str, job_name: str
) -> None:
    """Only the two generated reports precede the merge and publisher upload."""
    steps = _steps(_job(workflow_name, job_name))
    generation_steps = [
        step
        for step in steps
        if "/generate-coverage@" in str(step.get("uses", ""))
    ]
    outputs = [
        str((step.get("with") or {}).get("output-path"))
        for step in generation_steps
        if isinstance(step.get("with") or {}, dict)
    ]
    expected_outputs = sorted(ALLOWED_REPORTS)
    assert sorted(outputs) == expected_outputs, (
        f"expected generation outputs {expected_outputs!r}; got {sorted(outputs)!r}"
    )

    merge_step = _merge_step(steps)
    merge_index = steps.index(merge_step)
    assert all(steps.index(step) < merge_index for step in generation_steps), (
        f"expected merge index {merge_index} after generation indexes "
        f"{[steps.index(step) for step in generation_steps]!r}"
    )

    if workflow_name == "coverage-main.yml":
        upload_indexes = [
            index
            for index, step in enumerate(steps)
            if step.get("name") == "Upload coverage data to CodeScene"
        ]
        assert len(upload_indexes) == 1, (
            f"expected one coverage upload step; got {upload_indexes!r}"
        )
        assert merge_index < upload_indexes[0], (
            f"expected merge index {merge_index} before upload index "
            f"{upload_indexes[0]}"
        )


def test_pull_request_merge_remains_linux_only() -> None:
    """The pull-request workflow merges on its Linux matrix leg only."""
    merge_step = _merge_step(_steps(_job("ci.yml", "build-test")))
    expected_condition = "${{ matrix.platform == 'linux' }}"
    actual_condition = merge_step.get("if")
    assert actual_condition == expected_condition, (
        f"expected merge condition {expected_condition!r}; "
        f"got {actual_condition!r}"
    )


@pytest.fixture
def bash_executable() -> str:
    """Return Bash or skip behavioural checks when it is unavailable."""
    executable = shutil.which("bash")
    if executable is None:
        pytest.skip("bash is required to execute extracted workflow scripts")
    return executable


@pytest.mark.parametrize(
    ("workflow_name", "job_name"), WORKFLOW_CASES, ids=WORKFLOW_IDS
)
def test_merge_passes_exact_inputs_and_preserves_output(
    workflow_name: str,
    job_name: str,
    bash_executable: str,
    tmp_path: pathlib.Path,
) -> None:
    """A successful merge calls the pinned tool with the two exact reports."""
    _prepare_reports(tmp_path)

    script = _merge_script(workflow_name, job_name)
    result, argument_path = _run_script(
        script, tmp_path, bash_executable, LCOV_OUTPUT
    )

    assert result.returncode == 0, (
        f"expected merge success; got {result.returncode}: "
        f"{result.stdout}{result.stderr}"
    )
    expected_arguments = ["x", f"lcov-result-merger@{MERGER_VERSION}", MERGER_INPUT_PATTERN]
    actual_arguments = argument_path.read_text(encoding="utf-8").splitlines()
    assert actual_arguments == expected_arguments, (
        f"expected bun arguments {expected_arguments!r}; got {actual_arguments!r}"
    )
    actual_output = (tmp_path / "lcov.info").read_text(encoding="utf-8")
    assert actual_output == LCOV_OUTPUT, (
        f"expected merged output {LCOV_OUTPUT!r}; got {actual_output!r}"
    )


@pytest.mark.parametrize(
    "workflow_case", WORKFLOW_CASES, ids=WORKFLOW_IDS
)
@pytest.mark.parametrize(
    "invalid_report",
    [
        ("missing", "lcov-no-serde-saphyr.info"),
        ("empty", "lcov-serde-saphyr.info"),
        ("extra", "lcov-extra.info"),
    ],
    ids=("missing-report", "empty-report", "unexpected-report"),
)
def test_invalid_reports_fail_before_invoking_bun(
    workflow_case: tuple[str, str],
    invalid_report: tuple[str, str],
    bash_executable: str,
    tmp_path: pathlib.Path,
) -> None:
    """Missing, empty and unexpected reports fail before merger execution."""
    workflow_name, job_name = workflow_case
    _, affected_file = invalid_report
    _prepare_reports(tmp_path, invalid_report)

    result, argument_path = _run_script(
        _merge_script(workflow_name, job_name), tmp_path, bash_executable, LCOV_OUTPUT
    )

    assert result.returncode != 0, (
        f"expected invalid input {affected_file!r} to fail; "
        f"got return code {result.returncode}"
    )
    error_output = result.stdout + result.stderr
    assert affected_file in error_output, (
        f"expected failure output to name {affected_file!r}; got {error_output!r}"
    )
    assert "::error::" in error_output, (
        f"expected a workflow error annotation; got {error_output!r}"
    )
    assert not argument_path.exists(), (
        f"expected bun not to run for {affected_file!r}; "
        f"recorded arguments {argument_path.read_text(encoding='utf-8')!r}"
    )


@pytest.mark.parametrize(
    ("workflow_name", "job_name"), WORKFLOW_CASES, ids=WORKFLOW_IDS
)
def test_empty_merger_output_fails_after_the_tool_runs(
    workflow_name: str,
    job_name: str,
    bash_executable: str,
    tmp_path: pathlib.Path,
) -> None:
    """An empty result is rejected after the merger command succeeds."""
    _prepare_reports(tmp_path)

    result, argument_path = _run_script(
        _merge_script(workflow_name, job_name), tmp_path, bash_executable, ""
    )

    assert result.returncode != 0, (
        f"expected empty lcov.info to fail; got return code {result.returncode}"
    )
    error_output = result.stdout + result.stderr
    assert "lcov.info" in error_output, (
        f"expected failure output to name 'lcov.info'; got {error_output!r}"
    )
    assert "::error::" in error_output, (
        f"expected a workflow error annotation; got {error_output!r}"
    )
    assert argument_path.is_file(), (
        f"expected stub invocation file {str(argument_path)!r} to exist; "
        f"got {argument_path.is_file()}"
    )
