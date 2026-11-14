"""Property-test the merged source-record guard.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import shutil
import typing as typ

import pytest
from hypothesis import example, given, settings
from hypothesis import strategies as st

import coverage_merge_test as workflow_contract

#: Printable ASCII characters for generated source paths and LCOV-like lines.
ASCII_LCOV_TEXT: typ.Final[st.SearchStrategy[str]] = st.text(
    alphabet=st.characters(min_codepoint=32, max_codepoint=126), max_size=24
)

#: Generated lines include valid source records, malformed near misses and tags.
LCOV_OUTPUT_LINES: typ.Final[st.SearchStrategy[str]] = st.one_of(
    st.text(
        alphabet=st.characters(min_codepoint=32, max_codepoint=126),
        min_size=1,
        max_size=16,
    ).map(lambda source_path: f"SF:{source_path}"),
    st.sampled_from(("SF:", " SF:src/lib.rs", "TN:", "DA:1,1", "end_of_record")),
    ASCII_LCOV_TEXT,
)


@st.composite
def _lcov_merge_outputs(draw: st.DrawFn) -> str:
    """Generate bounded LCOV-like lines with LF and CRLF endings."""
    line_endings = st.sampled_from(("\n", "\r\n"))
    segments = draw(
        st.lists(st.tuples(LCOV_OUTPUT_LINES, line_endings), max_size=8)
    )
    final_ending = draw(st.sampled_from(("", "\n", "\r\n")))
    return "".join(line + ending for line, ending in segments) + final_ending


def _model_has_source_record(merged_output: str) -> bool:
    """Model an SF record with a non-whitespace path, preserving CR endings."""
    whitespace = " \t\r\v\f"
    return any(
        line.startswith("SF:")
        and any(character not in whitespace for character in line[3:])
        for line in merged_output.split("\n")
    )


@pytest.fixture(scope="module")
def bash_executable() -> str:
    """Return Bash or skip when it is unavailable."""
    executable = shutil.which("bash")
    if executable is None:
        pytest.skip("bash is required to execute extracted workflow scripts")
    return executable


@example(merged_output="SF:\r\n")
@example(merged_output="SF:\n")
@example(merged_output=" SF:src/lib.rs\n")
@example(merged_output="TN:\r\nDA:1,1\n")
@example(merged_output="TN:\nSF:src/lib.rs\r\n")
@settings(max_examples=30, deadline=None)
@given(merged_output=_lcov_merge_outputs())
def test_source_record_guard_matches_model_for_generated_output(
    merged_output: str,
    bash_executable: str,
    tmp_path_factory: pytest.TempPathFactory,
) -> None:
    """Both scripts accept generated output only when an SF path is present."""
    expected_acceptance = _model_has_source_record(merged_output)

    for workflow_name, job_name in workflow_contract.WORKFLOW_CASES:
        working_directory = tmp_path_factory.mktemp(
            f"coverage-merge-output-{workflow_name.removesuffix('.yml')}"
        )
        workflow_contract._prepare_reports(working_directory)
        result, _ = workflow_contract._run_script(
            workflow_contract._merge_script(workflow_name, job_name),
            working_directory,
            bash_executable,
            merged_output,
        )
        actual_acceptance = result.returncode == 0
        assert actual_acceptance == expected_acceptance, (
            f"expected source-record acceptance {expected_acceptance} for "
            f"{workflow_name} and output {merged_output!r}; got "
            f"{actual_acceptance}: {result.stdout}{result.stderr}"
        )
