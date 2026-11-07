"""Reproduce the child-cargo reader comparison between two Git revisions.

The script extracts both reader sets and the source tree named by the newer
revision, then compares class membership, per-marker membership, and every
source path and text reached from the contract's test roots.

Run from the repository root with:

    python3 -I scripts/verify_child_cargo_reader_equivalence.py \
        42c11d60 bb1fbe692f25fa3bc82792d2871863619021ac28

The revisions are explicit so a later ``HEAD`` cannot silently change the
comparison. The command needs no Python package beyond the standard library.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys
import tarfile
import tempfile
import typing as typ


#: Where the contract helpers live, relative to the repository root.
_READER_DIRECTORY: typ.Final[str] = "tests/workflow_contracts"


def _git_text(repository: pathlib.Path, *arguments: str) -> str:
    """Run one Git read command and return its text output.

    Examples
    --------
    From a repository checkout, resolve a commit name:

    >>> bool(_git_text(pathlib.Path.cwd(), "rev-parse", "--show-toplevel"))
    True
    """
    return subprocess.run(
        ["git", "-C", str(repository), *arguments],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.rstrip("\n")


def _revision(repository: pathlib.Path, reference: str) -> str:
    """Resolve a user-supplied name to one immutable commit ID."""
    return _git_text(repository, "rev-parse", "--verify", f"{reference}^{{commit}}")


def _extract_readers(
    repository: pathlib.Path, revision: str, destination: pathlib.Path
) -> None:
    """Write the comparison readers from ``revision`` into ``destination``.

    Every helper module at that revision is extracted rather than a pinned
    list, because the later revision's readers import modules the earlier one
    never had. A pinned list went stale as soon as ``child_cargo_tier`` was
    split, and the comparison then failed on an import rather than a reading.
    """
    destination.mkdir(parents=True)
    listing = _git_text(
        repository, "ls-tree", "-r", "--name-only", revision, "--", _READER_DIRECTORY
    )
    for relative in listing.splitlines():
        if not relative.endswith(".py"):
            continue
        contents = _git_text(repository, "show", f"{revision}:{relative}")
        (destination / pathlib.Path(relative).name).write_text(
            contents, encoding="utf-8"
        )


def _extract_source_tree(
    repository: pathlib.Path, revision: str, destination: pathlib.Path
) -> None:
    """Extract the newer revision's tracked source tree as the shared input."""
    destination.mkdir(parents=True)
    archive = subprocess.Popen(
        ["git", "-C", str(repository), "archive", "--format=tar", revision],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if archive.stdout is None:
        raise RuntimeError("git archive did not provide its output stream")
    with tarfile.open(fileobj=archive.stdout, mode="r|") as source:
        source.extractall(destination, filter="data")
    stderr = archive.stderr.read().decode("utf-8", errors="replace") if archive.stderr else ""
    status = archive.wait()
    if status:
        raise RuntimeError(f"git archive failed with exit {status}: {stderr.strip()}")


def _worker(repository: pathlib.Path, readers: pathlib.Path) -> None:
    """Serialize the readings produced by one isolated reader set."""
    sys.path.insert(0, str(readers))
    import child_cargo_tier as tier
    from rust_module_graph import sources_reached

    root_records: list[tuple[str, str, str]] = []
    reached_by_root: dict[str, dict[str, str]] = {}
    for crate in tier.CHILD_CARGO_TIER_CRATES:
        for name, root in tier._crate_test_roots(repository, crate):
            root_path = root.relative_to(repository).as_posix()
            root_records.append((crate, name, root_path))
            reached_by_root[root_path] = {
                source.relative_to(repository).as_posix(): contents
                for source, contents in sources_reached(root).items()
            }
    root_records.sort()
    snapshot = {
        "roots": root_records,
        "class": sorted(tier.child_cargo_binaries(repository)),
        "non_class": sorted(tier.non_child_cargo_binaries(repository)),
        "markers": {
            label: sorted(names)
            for label, names in sorted(tier.marker_binaries(repository).items())
        },
        "sources": reached_by_root,
    }
    print(json.dumps(snapshot, ensure_ascii=True, sort_keys=True))


def _read_snapshot(
    repository: pathlib.Path, source_tree: pathlib.Path, readers: pathlib.Path
) -> dict[str, typ.Any]:
    """Run one reader set in an isolated interpreter and parse its result."""
    command = [
        sys.executable,
        "-I",
        str(pathlib.Path(__file__).resolve()),
        "--worker",
        str(source_tree),
        str(readers),
    ]
    result = subprocess.run(
        command, cwd=repository, capture_output=True, text=True, check=False
    )
    if result.returncode:
        raise RuntimeError(
            f"reader worker failed ({result.returncode}): {result.stderr.strip()}"
        )
    return json.loads(result.stdout)


def _source_differences(
    older: dict[str, typ.Any], newer: dict[str, typ.Any]
) -> tuple[int, int]:
    """Count roots compared and roots with changed paths or source text."""
    old_sources = older["sources"]
    new_sources = newer["sources"]
    roots = set(old_sources) | set(new_sources)
    different = sum(old_sources.get(root) != new_sources.get(root) for root in roots)
    return len(set(old_sources) & set(new_sources)), different


def _report(
    older: dict[str, typ.Any], newer: dict[str, typ.Any], old_sha: str, new_sha: str
) -> bool:
    """Print equality evidence and return whether every reading agrees."""
    roots_compared, roots_differing = _source_differences(older, newer)
    markers_equal = older["markers"] == newer["markers"]
    class_equal = older["class"] == newer["class"]
    non_class_equal = older["non_class"] == newer["non_class"]
    roots_equal = older["roots"] == newer["roots"]
    print(f"older revision: {old_sha}")
    print(f"newer revision: {new_sha}")
    print(f"class equal: {class_equal} {len(newer['class'])}")
    print(f"non-class equal: {non_class_equal} {len(newer['non_class'])}")
    marker_counts = {label: len(names) for label, names in newer["markers"].items()}
    print(f"markers equal: {markers_equal} {marker_counts}")
    print(f"root inventory equal: {roots_equal}")
    print(f"roots compared: {roots_compared}")
    print(f"roots differing (paths and text): {roots_differing}")
    return all(
        (class_equal, non_class_equal, markers_equal, roots_equal, roots_differing == 0)
    )


def _main(arguments: list[str] | None = None) -> int:
    """Extract two revisions and compare their child-cargo readings.

    Examples
    --------
    Pin both revisions when reproducing a prior comparison:

    >>> "42c11d60" in "python3 -I verify.py 42c11d60 bb1fbe69"
    True
    """
    values = list(sys.argv[1:] if arguments is None else arguments)
    if values[:1] == ["--worker"]:
        if len(values) != 3:
            raise SystemExit("worker mode requires a source tree and reader directory")
        _worker(pathlib.Path(values[1]), pathlib.Path(values[2]))
        return 0

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("older_revision", help="commit before the reader refactor")
    parser.add_argument("newer_revision", help="commit containing the refactored readers")
    parser.add_argument(
        "--repository", default=".", help="Git checkout containing both revisions"
    )
    options = parser.parse_args(values)
    repository = pathlib.Path(options.repository).resolve()
    old_sha = _revision(repository, options.older_revision)
    new_sha = _revision(repository, options.newer_revision)

    with tempfile.TemporaryDirectory(prefix="child-cargo-reader-equivalence-") as temporary:
        temporary_root = pathlib.Path(temporary)
        source_tree = temporary_root / "source"
        _extract_source_tree(repository, new_sha, source_tree)
        snapshots: list[dict[str, typ.Any]] = []
        for label, revision in (("old", old_sha), ("new", new_sha)):
            readers = temporary_root / label
            _extract_readers(repository, revision, readers)
            snapshots.append(_read_snapshot(repository, source_tree, readers))

    if _report(snapshots[0], snapshots[1], old_sha, new_sha):
        return 0
    return 1


if __name__ == "__main__":
    raise SystemExit(_main())
