"""Exercise the pinned merger's real output and empty-record behaviour."""

from __future__ import annotations

import os
import pathlib
import shlex
import signal
import shutil
import subprocess
import time

import pytest

import coverage_merge_test as workflow_contract

#: Maximum time allowed for either real merger execution.
MERGER_TIMEOUT_SECONDS = 180

#: Grace period before a timed-out process tree is forcefully stopped.
PROCESS_TERMINATION_GRACE_SECONDS = 5

#: Distinct valid LCOV records expected to survive a real merge.
VALID_REPORTS: dict[str, str] = {
    "lcov-serde-saphyr.info": (
        "TN:\nSF:src/serde.rs\nDA:1,1\nend_of_record\n"
    ),
    "lcov-no-serde-saphyr.info": (
        "TN:\nSF:src/no_serde.rs\nDA:2,1\nend_of_record\n"
    ),
}


def _stop_process_tree(process: subprocess.Popen[str]) -> None:
    """Stop a timed-out Bash process and any merger children it started."""
    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    else:
        subprocess.run(
            ["taskkill", "/F", "/T", "/PID", str(process.pid)],
            check=False,
            capture_output=True,
            text=True,
        )

    try:
        process.communicate(timeout=PROCESS_TERMINATION_GRACE_SECONDS)
    except subprocess.TimeoutExpired:
        if os.name == "posix":
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        else:
            subprocess.run(
                ["taskkill", "/F", "/T", "/PID", str(process.pid)],
                check=False,
                capture_output=True,
                text=True,
            )
        try:
            process.communicate(timeout=PROCESS_TERMINATION_GRACE_SECONDS)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=PROCESS_TERMINATION_GRACE_SECONDS)
            for stream in (process.stdin, process.stdout, process.stderr):
                if stream is not None:
                    stream.close()


def _run_bash(
    script: str,
    working_directory: pathlib.Path,
    path: str,
    timeout_seconds: float = MERGER_TIMEOUT_SECONDS,
) -> subprocess.CompletedProcess[str]:
    """Run Bash with bounded cleanup for the shell and its descendants."""
    bash_executable = shutil.which("bash")
    if bash_executable is None:
        pytest.skip("bash is required to execute extracted workflow scripts")

    environment = os.environ.copy()
    environment["PATH"] = path
    process = subprocess.Popen(
        [bash_executable],
        cwd=working_directory,
        env=environment,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=os.name == "posix",
        text=True,
    )
    try:
        stdout, stderr = process.communicate(
            input=script, timeout=timeout_seconds
        )
    except BaseException:
        _stop_process_tree(process)
        raise
    return subprocess.CompletedProcess(
        process.args, process.returncode, stdout, stderr
    )


def _run_real_merger(
    script: str, working_directory: pathlib.Path
) -> subprocess.CompletedProcess[str]:
    """Run an extracted workflow script with the installed Bun executable."""
    bun_executable = shutil.which("bun")
    if bun_executable is None:
        pytest.skip("Bun is required to execute the pinned real merger")
    path = os.pathsep.join(
        (str(pathlib.Path(bun_executable).parent), os.environ.get("PATH", os.defpath))
    )
    return _run_bash(script, working_directory, path)


@pytest.mark.skipif(
    os.name != "posix", reason="the process-group timeout check requires POSIX"
)
def test_bash_timeout_terminates_its_process_group(tmp_path: pathlib.Path) -> None:
    """A timed-out Bash script cannot leave its child writing in tmp_path."""
    marker_path = tmp_path / "surviving-child.txt"
    script = (
        f"( sleep 1; printf alive > {shlex.quote(str(marker_path))} ) &\n"
        "wait\n"
    )

    with pytest.raises(subprocess.TimeoutExpired):
        _run_bash(
            script,
            tmp_path,
            os.environ.get("PATH", os.defpath),
            timeout_seconds=0.2,
        )

    time.sleep(1.1)
    marker_contents = (
        marker_path.read_text(encoding="utf-8") if marker_path.exists() else None
    )
    assert not marker_path.exists(), (
        f"expected process cleanup to stop the child before it writes; "
        f"got {marker_contents!r}"
    )


@pytest.mark.parametrize(
    "workflow_case", workflow_contract.WORKFLOW_CASES,
    ids=workflow_contract.WORKFLOW_IDS,
)
def test_real_merger_newline_without_records_fails(
    workflow_case: tuple[str, str], tmp_path: pathlib.Path
) -> None:
    """The pinned merger's newline-only result fails the record guard."""
    workflow_name, job_name = workflow_case
    workflow_contract._prepare_reports(tmp_path)

    result = _run_real_merger(
        workflow_contract._merge_script(workflow_name, job_name), tmp_path
    )

    assert result.returncode != 0, (
        f"expected a report with no coverage records to fail; got "
        f"{result.returncode}: {result.stdout}{result.stderr}"
    )
    merged_output = (tmp_path / "lcov.info").read_text(encoding="utf-8")
    assert merged_output == "\n", (
        f"expected the real merger to emit one newline; got {merged_output!r}"
    )
    error_output = result.stdout + result.stderr
    assert "::error::" in error_output and "lcov.info" in error_output, (
        f"expected a named workflow error annotation; got {error_output!r}"
    )


@pytest.mark.parametrize(
    "workflow_case", workflow_contract.WORKFLOW_CASES,
    ids=workflow_contract.WORKFLOW_IDS,
)
def test_real_merger_preserves_records_from_both_reports(
    workflow_case: tuple[str, str], tmp_path: pathlib.Path
) -> None:
    """A valid real merge retains distinct records from both input reports."""
    workflow_name, job_name = workflow_case
    for report, contents in VALID_REPORTS.items():
        (tmp_path / report).write_text(contents, encoding="utf-8")

    result = _run_real_merger(
        workflow_contract._merge_script(workflow_name, job_name), tmp_path
    )

    assert result.returncode == 0, (
        f"expected valid reports to merge; got {result.returncode}: "
        f"{result.stdout}{result.stderr}"
    )
    merged_output = (tmp_path / "lcov.info").read_text(encoding="utf-8")
    merged_records = merged_output.split("end_of_record")
    expected_records = {
        "SF:src/serde.rs": "DA:1,1",
        "SF:src/no_serde.rs": "DA:2,1",
    }
    for source_record, data_record in expected_records.items():
        record = next(
            (chunk for chunk in merged_records if source_record in chunk), None
        )
        assert record is not None, (
            f"expected merged output to include {source_record!r}; "
            f"got {merged_output!r}"
        )
        assert data_record in record, (
            f"expected {source_record!r} to retain {data_record!r}; "
            f"got {record!r}"
        )
    assert merged_output.count("end_of_record") == len(expected_records), (
        f"expected one merged record for each input; got {merged_output!r}"
    )
