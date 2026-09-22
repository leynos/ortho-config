"""The CV-005 readers, driven on documents this repository does not have.

Every rule in ``codescene_coverage_test`` derives its subject from these
readings, and each of those rules is a refusal. A refusal over an empty
subject set is satisfied by any repository at all, so a reader that
quietly finds nothing does not report an error and does not report zero:
it reports compliance.

The real workflows cannot catch that. They use one spelling of ``on:``,
one push filter and one job shape, so they exercise one path through
each reader and agree with a broken one as readily as with a working
one. Every case here is constructed for that reason.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import pytest
import yaml
from codescene_coverage import (
    WorkflowReadingError,
    load_workflow,
    publishers,
    pull_request_workflows,
    pushes_to_main,
    read_workflows,
    serves_pull_requests,
    triggers,
    workflow_steps,
)

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    import pathlib

#: A minimal workflow body, so each case below varies one thing.
JOBS: typ.Final[str] = "jobs:\n  a:\n    steps: []\n"


@pytest.mark.parametrize(
    ("document", "expected"),
    [
        pytest.param("on:\n  pull_request:\n", {"pull_request"}, id="mapping"),
        pytest.param("'on':\n  pull_request:\n", {"pull_request"}, id="quoted-key"),
        pytest.param("on: [pull_request, push]", {"pull_request", "push"}, id="list"),
        pytest.param("on: pull_request", {"pull_request"}, id="bare-string"),
        pytest.param("name: x\n", set(), id="no-triggers"),
    ],
)
def test_the_trigger_reader_handles_every_spelling(
    document: str, expected: set[str]
) -> None:
    """Assert the reader on documents this repository does not contain.

    Every rule built on it derives its subject from the result, so a
    reader returning nothing makes all of them pass over an empty set.
    """
    assert triggers(load_workflow(document)) == expected, (
        f"the reader must find {sorted(expected)} in {document!r}"
    )


def test_the_trigger_reader_survives_a_resolving_loader() -> None:
    """The ``on`` key can arrive as a boolean, and the reader must cope.

    YAML 1.1 resolves an unquoted ``on:`` to the boolean ``True``, so a
    loader that resolves scalars keys every workflow in this estate
    under ``True`` and none under ``on``. ``load_workflow`` uses
    ``yaml.BaseLoader`` and keeps the string, which is exactly why this
    case is written against ``yaml.safe_load`` instead: the hazard is a
    property of the loader, and swapping ``load_workflow`` to a
    resolving one is a one-word change that would otherwise empty every
    rule in the neighbouring module while every assertion still passed.

    Without this case, narrowing the reader to the string key alone
    fails nothing at all. Measured: that mutation left the whole
    contract green.
    """
    document = yaml.safe_load("on:\n  pull_request:\n  push:\n    branches: [main]\n")
    assert True in document, (
        "this case only means something while a resolving loader keys an "
        "unquoted `on:` under the boolean; if PyYAML changes, delete it"
    )
    assert triggers(document) == {"pull_request", "push"}, (
        "the reader must find the triggers under the boolean key too"
    )


@pytest.mark.parametrize(
    ("filters", "expected", "why"),
    [
        pytest.param("", True, "a bare push runs for every branch", id="bare"),
        pytest.param(
            "\n    branches: [main]", True, "main is named", id="branches-main"
        ),
        pytest.param(
            "\n    branches: main", True, "a scalar names main", id="branches-scalar"
        ),
        pytest.param(
            "\n    branches: [release]", False, "main is not named", id="branches-other"
        ),
        pytest.param(
            "\n    branches-ignore: [main]",
            False,
            "main is excluded",
            id="branches-ignore-main",
        ),
        pytest.param(
            "\n    branches-ignore: [wip]",
            True,
            "main is not excluded",
            id="branches-ignore-other",
        ),
        pytest.param(
            "\n    tags: ['v*']", False, "a tag push is not a branch push", id="tags"
        ),
        pytest.param(
            "\n    tags-ignore: ['v*']",
            False,
            "a tag filter alone still leaves only tag pushes",
            id="tags-ignore",
        ),
        pytest.param(
            "\n    paths: ['src/**']",
            True,
            "a path filter narrows without excluding main",
            id="paths",
        ),
    ],
)
def test_the_push_reader_answers_every_filter_form(
    filters: str, *, expected: bool, why: str
) -> None:
    """Which workflow may publish turns on this reading.

    ``publishers`` grants the CodeScene upload to a workflow that pushes
    to main, so a reader answering True for a shape that never runs on a
    main push hands that permission to the wrong file, and the contract
    passes while the wrong workflow uploads.

    The tag forms are the ones a naive reading gets wrong. A ``push``
    filtered to tags alone fires for tag pushes and never for a branch.
    """
    document = load_workflow(f"on:\n  push:{filters}\n{JOBS}")
    assert pushes_to_main(document) is expected, (
        f"push{filters!r}: {why}, so the reader must answer {expected}"
    )


def test_a_workflow_with_no_push_trigger_never_publishes() -> None:
    """Assert the push reader is narrow as well as broad.

    A reading that answered True whenever it could not find a reason to
    say no would satisfy most of the table above and make every
    pull-request workflow a candidate publisher.
    """
    assert not pushes_to_main(load_workflow(f"on:\n  pull_request:\n{JOBS}")), (
        "a workflow declaring no push trigger cannot push to main"
    )


def test_a_publisher_serving_pull_requests_is_not_a_publisher() -> None:
    """Both halves of the predicate, and the second is the one dropped.

    A repository's main workflow often declares ``pull_request`` and
    ``push: branches: [main]`` together. A predicate reading only the
    push makes that one file simultaneously required to upload and
    forbidden from uploading, so the contract contradicts itself rather
    than failing.
    """
    both = load_workflow(f"on:\n  pull_request:\n  push:\n    branches: [main]\n{JOBS}")
    assert serves_pull_requests(both), "the fixture must serve pull requests"
    assert publishers({"ci.yml": both}) == {}, (
        "a workflow serving pull requests is not a publisher, whatever else "
        "triggers it"
    )


@pytest.mark.parametrize(
    "document",
    [
        pytest.param("- a\n- b\n", id="a-list"),
        pytest.param("just a string\n", id="a-scalar"),
        pytest.param("", id="empty"),
    ],
)
def test_a_document_that_is_not_a_workflow_is_refused(document: str) -> None:
    """A workflow is a mapping, and anything else is not one.

    Returning an empty mapping instead would make every reading over it
    find nothing, which each rule built on them reads as compliance.
    """
    with pytest.raises(TypeError, match="top-level mapping"):
        load_workflow(document)


def test_an_empty_workflow_directory_is_a_reader_fault(
    tmp_path: pathlib.Path,
) -> None:
    """Finding no workflow at all is never an answer.

    The fault names the reading and the directory rather than leaving a
    caller to parse a message.
    """
    with pytest.raises(WorkflowReadingError) as raised:
        read_workflows(tmp_path)
    assert raised.value.reader == "read_workflows", raised.value
    assert raised.value.path == str(tmp_path), raised.value


def test_an_unparseable_workflow_names_its_file(tmp_path: pathlib.Path) -> None:
    """A file that is not a workflow is reported with its name.

    Left to escape, it surfaces as a ``TypeError`` from inside a helper
    whose name and return type promise a mapping, with nothing saying
    which of the directory's files it came from.
    """
    (tmp_path / "broken.yml").write_text("- not a mapping\n", encoding="utf-8")
    with pytest.raises(WorkflowReadingError) as raised:
        read_workflows(tmp_path)
    assert raised.value.reader == "read_workflows", raised.value
    assert "broken.yml" in (raised.value.path or ""), raised.value


def test_no_pull_request_workflow_is_a_reader_fault() -> None:
    """The same argument one layer up.

    A repository with a pull-request lane that reads as having none is a
    broken reading, and the rules refusing things on that lane would all
    pass over the empty set.
    """
    documents = {"main-only.yml": load_workflow(f"on:\n  push:\n{JOBS}")}
    with pytest.raises(WorkflowReadingError) as raised:
        pull_request_workflows(documents)
    assert raised.value.reader == "pull_request_workflows", raised.value


@pytest.mark.parametrize(
    "body",
    [
        pytest.param("jobs: not-a-mapping\n", id="jobs-not-a-mapping"),
        pytest.param("jobs:\n  a: not-a-mapping\n", id="job-not-a-mapping"),
        pytest.param("jobs:\n  a:\n    steps: not-a-list\n", id="steps-not-a-list"),
        pytest.param("jobs:\n  a:\n    steps:\n      - a-string\n", id="step-a-string"),
        pytest.param("name: x\n", id="no-jobs"),
    ],
)
def test_a_malformed_job_shape_yields_no_steps(body: str) -> None:
    """A fragment this reading cannot use contributes nothing.

    Refusing the whole document instead would let a workflow with
    nothing to do with coverage fail a contract about coverage, which is
    the wrong failure in the wrong place.
    """
    assert workflow_steps(load_workflow(f"on:\n  push:\n{body}")) == [], (
        f"a malformed job shape yields no steps: {body!r}"
    )
