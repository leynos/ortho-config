"""Exercise the pinned merger's real output and empty-record behaviour."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess

import pytest

import coverage_merge_test as workflow_contract

#: Distinct valid LCOV records expected to survive a real merge.
VALID_REPORTS: dict[str, str] = {
    "lcov-serde-saphyr.info": (
        "TN:\nSF:src/serde.rs\nDA:1,1\nend_of_record\n"
    ),
    "lcov-no-serde-saphyr.info": (
        "TN:\nSF:src/no_serde.rs\nDA:2,1\nend_of_record\n"
    ),
}


def _run_real_merger(script: str, working_directory: pathlib.Path) -> subprocess.CompletedProcess[str]:
    """Run an extracted workflow script with the installed Bun executable."""
    bash_executable = shutil.which("bash")
    if bash_executable is None:
        pytest.skip("bash is required to execute extracted workflow scripts")
    bun_executable = shutil.which("bun")
    if bun_executable is None:
        pytest.skip("Bun is required to execute the pinned real merger")

    environment = os.environ.copy()
    environment["PATH"] = os.pathsep.join(
        (str(pathlib.Path(bun_executable).parent), environment.get("PATH", os.defpath))
    )
    return subprocess.run(
        [bash_executable],
        input=script,
        cwd=working_directory,
        env=environment,
        capture_output=True,
        check=False,
        text=True,
        timeout=180,
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
