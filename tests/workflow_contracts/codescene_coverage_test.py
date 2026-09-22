"""CV-005: CodeScene coverage belongs to main, and to main alone.

Concordat rule ``main-owned-codescene-coverage``. One workflow uploads
coverage to CodeScene, it is the one that runs on pushes to main, and no
workflow serving pull requests names a CodeScene action, invokes
``cs-coverage``, or puts ``CS_ACCESS_TOKEN`` in reach of any process.

The rule is a policy rather than a gap. A pull request from a fork
cannot read ``CS_ACCESS_TOKEN``, so a changed-line check on that lane
was a silent skip for exactly the contributions least likely to have
been measured. On a branch it put a second tool on the critical path,
and when the CodeScene project stopped returning a gates configuration
that tool failed every pull request here over a defect in none of them.
The ratchet applies the same gate from this repository's own baseline
and needs no token.

What a pull-request lane keeps is ``generate-coverage`` with
``with-ratchet``, against the baseline the main publisher writes. This
repository runs two feature legs per lane, paired by ``output-path``,
and the pair that ratchets has to match field for field: the ratchet
compares this commit's report with that baseline, so a feature compiled
on one side and not the other makes the comparison measure the
difference between two builds rather than between two commits.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import typing as typ

import pytest
from codescene_coverage import (
    CLI_COMMAND,
    CODESCENE_ACTION,
    COVERAGE_ACTION,
    PINNED_COMMIT,
    FORBIDDEN_VARIABLE,
    coverage_steps,
    publishers,
    pull_request_workflows,
    read_workflows,
    token_sites,
    workflow_steps,
)

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from codescene_coverage import WorkflowDocument

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".github" / "workflows"

#: The repository variable this adoption retires. ``installer-checksum``
#: is rejected when non-empty from the pinned uploader, and the value
#: this variable holds is the installer script's digest rather than the
#: manifest archive's, so it cannot be carried across under
#: ``archive-checksum`` either.
RETIRED_VARIABLE: typ.Final[str] = "CODESCENE_CLI_SHA256"

#: The ref the publisher's upload step must be guarded on.
MAIN_REF: typ.Final[str] = "github.ref == 'refs/heads/main'"

#: The inputs that decide what a coverage run measures, as opposed to
#: what happens to the report afterwards. ``artefact-name-suffix`` is
#: excluded deliberately: the two lanes differ on it so their artefacts
#: do not collide, and it changes nothing about what is compiled.
SELECTION: typ.Final[frozenset[str]] = frozenset(
    {"output-path", "format", "features", "with-default-features", "all-features"}
)


@pytest.fixture(scope="module")
def documents() -> dict[str, WorkflowDocument]:
    """Return this repository's workflows, parsed once.

    Returns
    -------
    dict
        File name to parsed document.
    """
    return read_workflows(WORKFLOWS)


def test_no_pull_request_workflow_names_the_codescene_action(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The upload action belongs to the main publisher alone.

    Matched on the action's path rather than on the word `CodeScene`,
    because both workflows discuss the policy in prose and a comment
    explaining why a step is absent must not read as the step.
    """
    offenders = sorted(
        f"{name}: {step.get('uses')}"
        for name, document in pull_request_workflows(documents).items()
        for step in workflow_steps(document)
        if CODESCENE_ACTION in str(step.get("uses", ""))
    )
    assert not offenders, (
        f"these pull-request lanes invoke {CODESCENE_ACTION}; CV-005 puts "
        f"the upload and the changed-line check on the push-to-main "
        f"publisher alone: {offenders}"
    )


def test_no_pull_request_workflow_runs_the_cli(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The action is not the only way to reach the tool.

    A ``run:`` step invoking ``cs-coverage`` directly is the same gate
    wearing different clothes, and a rule naming only the action would
    read it as compliance.
    """
    offenders = sorted(
        f"{name}: {str(step.get('run', ''))[:60]}"
        for name, document in pull_request_workflows(documents).items()
        for step in workflow_steps(document)
        if CLI_COMMAND in str(step.get("run", ""))
    )
    assert not offenders, (
        f"these pull-request lanes run {CLI_COMMAND} directly: {offenders}"
    )


def test_no_pull_request_workflow_receives_the_token(
    documents: dict[str, WorkflowDocument],
) -> None:
    """A token no fork can read is a gate no fork is held to.

    Swept at workflow, job and step level, because all three reach a
    process, and structurally rather than over the raw file: both
    workflows explain in prose why the check is gone, and a comment is
    not a token.
    """
    offenders = sorted(
        site
        for name, document in pull_request_workflows(documents).items()
        for site in token_sites(name, document)
    )
    assert not offenders, (
        f"these pull-request lanes put {FORBIDDEN_VARIABLE} in reach; a "
        f"fork cannot read it, so the gate it guards is skipped for exactly "
        f"the contributions least likely to have been measured: {offenders}"
    )


def test_exactly_one_workflow_publishes_coverage(
    documents: dict[str, WorkflowDocument],
) -> None:
    """One publisher, so the baseline has one writer.

    Two would race on the ratchet baseline, and which one a pull request
    compared against would depend on which finished last. None would
    leave every pull request ratcheting against a baseline nobody
    writes, which passes silently and measures nothing.
    """
    found = publishers(documents)
    uploading = sorted(
        name
        for name, document in documents.items()
        for step in workflow_steps(document)
        if CODESCENE_ACTION in str(step.get("uses", ""))
    )
    assert len(found) == 1, (
        f"expected exactly one workflow that pushes to main and serves no "
        f"pull request; found {sorted(found)}"
    )
    assert uploading == sorted(found), (
        f"the workflows invoking {CODESCENE_ACTION} must be exactly the "
        f"publisher; the publisher is {sorted(found)} and the uploaders "
        f"are {uploading}"
    )


def _publisher_upload(documents: dict[str, WorkflowDocument]) -> dict[str, object]:
    """Return the publisher's single CodeScene upload step."""
    ((name, document),) = publishers(documents).items()
    steps = [
        step
        for step in workflow_steps(document)
        if CODESCENE_ACTION in str(step.get("uses", ""))
    ]
    assert len(steps) == 1, (
        f"{name} must invoke {CODESCENE_ACTION} once; it invokes it "
        f"{len(steps)} times"
    )
    return steps[0]


def test_the_publisher_uploads_rather_than_checks(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The mode is named, not left to the action's default.

    ``mode`` decides whether the step uploads a report or gates a pull
    request against one, and the default has changed before. Naming it
    is how a reader of these lines knows which of the two this step does
    without reading the action.
    """
    inputs = _publisher_upload(documents).get("with") or {}
    assert isinstance(inputs, dict), f"the upload step's `with:` is {inputs!r}"
    assert inputs.get("mode") == "upload", (
        f"the publisher's CodeScene step must name `mode: upload`; it names "
        f"{inputs.get('mode')!r}"
    )


def test_the_publisher_passes_no_deprecated_checksum(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The old checksum input fails the run outright.

    ``installer-checksum`` is rejected when non-empty from this pin, and
    ``archive-checksum`` is not a rename of it: it digests the action's
    CLI manifest archive, while the repository variable the old input
    carried holds the installer script's digest. Carrying the value
    across under the new name would fail every run, so neither input is
    passed and the action's own manifest pins the CLI instead.
    """
    inputs = _publisher_upload(documents).get("with") or {}
    assert isinstance(inputs, dict), f"the upload step's `with:` is {inputs!r}"
    for rejected in ("installer-checksum", "archive-checksum"):
        assert rejected not in inputs, (
            f"the publisher passes {rejected!r}; `installer-checksum` is "
            f"rejected when non-empty and `archive-checksum` digests a "
            f"different artefact from the variable this repository holds"
        )


def test_the_publisher_uploads_only_from_main(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The token is not enough; the ref has to be checked too.

    The publisher declares no ``workflow_dispatch`` today, so this guard
    changes nothing now. It is asserted because a dispatch can be aimed
    at any branch and the upload carries no ref, so adding one later
    would otherwise let a run from a feature branch publish that
    branch's coverage as the trunk's, silently, and every pull request
    would then ratchet against it.
    """
    condition = str(_publisher_upload(documents).get("if", ""))
    assert MAIN_REF in condition, (
        f"the publisher's upload step must be guarded on {MAIN_REF}; it is "
        f"guarded on {condition!r}"
    )


def test_the_publisher_runs_one_at_a_time(
    documents: dict[str, WorkflowDocument],
) -> None:
    """Two publisher runs racing decide the baseline by finishing order.

    The baseline this workflow writes is what every pull request
    ratchets against, so a superseded run finishing last makes its
    figures the trunk's.
    """
    ((name, document),) = publishers(documents).items()
    concurrency = document.get("concurrency")
    assert isinstance(concurrency, dict), (
        f"{name} must declare a concurrency block; two publisher runs "
        f"otherwise race on the ratchet baseline. It declares {concurrency!r}"
    )
    assert concurrency.get("group"), (
        f"{name}'s concurrency block must name a group: {concurrency!r}"
    )
    assert concurrency.get("cancel-in-progress") == "true", (
        f"{name} must cancel a superseded publisher run; without it the "
        f"older run can still write the baseline after the newer one"
    )


def test_every_coverage_lane_names_one_pinned_commit(
    documents: dict[str, WorkflowDocument],
) -> None:
    """One commit across the repository, and a commit rather than a branch.

    The SHA is not named. ``AGENTS.md`` is explicit that a contract
    asserting a caller's exact commit makes every Dependabot bump a
    manual edit, so what is held here is the shape and the agreement:
    each reference is pinned to a full forty-character commit, and every
    coverage lane names the same one.

    The agreement is what matters for the ratchet. A pull request's
    coverage is compared against the baseline main published, so two
    lanes running different versions of the generator is a comparison
    between two tools, and a partial repin is invisible in a diff that
    moves the other lane.
    """
    steps = coverage_steps(documents)
    assert steps, "no workflow invokes the coverage action; the reader is broken"
    pins: dict[str, set[str]] = {}
    for name, lane in steps.items():
        for step in lane:
            reference = str(step.get("uses", ""))
            prefix = f"{COVERAGE_ACTION}@"
            assert reference.startswith(prefix), (
                f"{name} must call {COVERAGE_ACTION} by path; it calls "
                f"{reference!r}"
            )
            pin = reference[len(prefix) :]
            assert PINNED_COMMIT.match(pin), (
                f"{name} pins {pin!r}, which is not a full forty-character "
                f"commit; a branch or a tag moves under the workflow"
            )
            pins.setdefault(name, set()).add(pin)
    distinct = {pin for lane in pins.values() for pin in lane}
    assert len(distinct) == 1, (
        f"the coverage lanes name different commits, so a pull request's "
        f"ratchet would compare reports built by two versions of the "
        f"generator: {pins}"
    )


def _by_output_path(
    steps: list[dict[str, object]],
) -> dict[str, dict[str, object]]:
    """Return one lane's coverage steps keyed by the report they write."""
    found: dict[str, dict[str, object]] = {}
    for step in steps:
        inputs = step.get("with") or {}
        assert isinstance(inputs, dict), f"a coverage step's `with:` is {inputs!r}"
        output = str(inputs.get("output-path", ""))
        assert output, f"a coverage step declares no output-path: {inputs!r}"
        assert output not in found, (
            f"two coverage steps write {output!r} in one lane, so this "
            f"reading cannot pair them with the publisher's"
        )
        found[output] = step
    return found


def test_the_ratcheting_lane_matches_its_baseline(
    documents: dict[str, WorkflowDocument],
) -> None:
    """A ratchet against a differently built baseline measures the builds.

    This repository runs two feature legs per lane. They are paired by
    the report each writes, and the inputs that decide *what* is
    measured must agree across the pair: the output path, the format and
    the feature selection.

    This was wrong before the adoption. The pull-request leg compiled
    `metrics` and the publisher's did not, so every line of the metrics
    facade read as newly uncovered against a baseline that had never
    compiled it. The failure is silent: the ratchet reports a number
    either way, so nothing distinguishes a fall in coverage from two
    runs having built different code.
    """
    steps = coverage_steps(documents)
    (publisher,) = publishers(documents)
    baseline = _by_output_path(steps[publisher])
    for name, lane in steps.items():
        if name == publisher:
            continue
        for output, step in _by_output_path(lane).items():
            assert output in baseline, (
                f"{name} writes {output!r}, which {publisher} does not, so "
                f"that leg ratchets against a baseline nobody publishes"
            )
            theirs = {
                key: value
                for key, value in (step.get("with") or {}).items()
                if key in SELECTION
            }
            ours = {
                key: value
                for key, value in (baseline[output].get("with") or {}).items()
                if key in SELECTION
            }
            assert theirs == ours, (
                f"{name}'s {output!r} leg is built differently from "
                f"{publisher}'s, so the ratchet would compare two builds "
                f"rather than two commits: {theirs} against {ours}"
            )


def test_no_workflow_reads_the_retired_variable() -> None:
    """The variable this adoption retires must have no readers left.

    Asserted on expressions rather than on the word. The publisher
    explains in prose why the checksum input is gone, and that
    explanation names the variable; a sweep over the raw text would read
    the explanation as a reader and push the next person into deleting
    the reason rather than the reference.
    """
    import re

    reference = re.compile(
        rf"\$\{{\{{[^}}]*\b(?:vars|env|secrets)\.{re.escape(RETIRED_VARIABLE)}\b"
    )
    offenders = sorted(
        path.name
        for pattern in ("*.yml", "*.yaml")
        for path in sorted(WORKFLOWS.glob(pattern))
        if reference.search(path.read_text(encoding="utf-8"))
    )
    assert not offenders, (
        f"these workflows still read {RETIRED_VARIABLE}; the uploader "
        f"rejects the input it fed and the action pins the CLI through its "
        f"own manifest now: {offenders}"
    )
