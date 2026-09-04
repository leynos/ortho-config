"""Contract tests for the release and packaging workflows.

Release workflows only run on a tag, so a mistake in one is discovered when a
release is already half-published. These tests parse `release.yml` and the
packaging job in `ci.yml` with PyYAML and pin the properties that have broken
releases elsewhere in the estate: a `gh` call with no repository context, a
job that reads draft assets with a read-scoped token, an upload that omits the
checksum sidecar, and an archive layout that no longer renders the crate's
`[package.metadata.binstall]` templates.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import sys
import tomllib
import typing as typ
from pathlib import Path

import pytest
import yaml

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_DIRECTORY = REPOSITORY_ROOT / "scripts"
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import release_archive_naming as naming  # noqa: E402

RELEASE_WORKFLOW_PATH = REPOSITORY_ROOT / ".github" / "workflows" / "release.yml"
CI_WORKFLOW_PATH = REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml"
MANIFEST_PATH = REPOSITORY_ROOT / "cargo-orthohelp" / "Cargo.toml"

PACKAGE_NAME = "cargo-orthohelp"

#: Targets the release publishes, mapped to the runner that builds each one
#: natively. Cross-compilation is deliberately avoided: every triple has a
#: GitHub-hosted runner of its own architecture and operating system.
EXPECTED_TARGET_RUNNERS = {
    "x86_64-unknown-linux-gnu": "ubuntu-24.04",
    "aarch64-unknown-linux-gnu": "ubuntu-24.04-arm",
    "x86_64-apple-darwin": "macos-15-intel",
    "aarch64-apple-darwin": "macos-latest",
    "x86_64-pc-windows-msvc": "windows-latest",
}

#: Jobs that create, upload to, or read the draft release. Draft releases are
#: invisible to read-scoped tokens, so the auditing job needs write scope too.
RELEASE_WRITING_JOBS = (
    "create-release",
    "build-assets",
    "audit-draft-assets",
    "publish-release",
)

#: OS families the pull-request packaging job must cover, so a layout or
#: sidecar regression fails before a release rather than during one.
EXPECTED_PACKAGING_RUNNERS = frozenset({"ubuntu-latest", "macos-latest", "windows-latest"})


def _load_workflow(path: Path) -> dict[str, typ.Any]:
    """Return the parsed workflow document at *path*."""
    return yaml.safe_load(path.read_text(encoding="utf-8"))


@pytest.fixture(name="release_workflow", scope="module")
def release_workflow_fixture() -> dict[str, typ.Any]:
    """Return the parsed release workflow."""
    return _load_workflow(RELEASE_WORKFLOW_PATH)


@pytest.fixture(name="ci_workflow", scope="module")
def ci_workflow_fixture() -> dict[str, typ.Any]:
    """Return the parsed CI workflow."""
    return _load_workflow(CI_WORKFLOW_PATH)


@pytest.fixture(name="binstall_metadata", scope="module")
def binstall_metadata_fixture() -> dict[str, str]:
    """Return the crate's `[package.metadata.binstall]` table."""
    return naming.binstall_metadata(MANIFEST_PATH)


def _steps(job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return the job's steps."""
    return list(job.get("steps") or [])


def _step_index(job: dict[str, typ.Any], predicate: typ.Callable[[dict], bool]) -> int:
    """Return the index of the first step satisfying *predicate*, or -1."""
    return next((index for index, step in enumerate(_steps(job)) if predicate(step)), -1)


def _invokes_gh(step: dict[str, typ.Any]) -> bool:
    """Return whether *step* shells out to the GitHub CLI."""
    run = step.get("run")
    return isinstance(run, str) and "gh " in run


def _has_checkout(job: dict[str, typ.Any]) -> bool:
    """Return whether *job* checks the repository out."""
    return any(str(step.get("uses", "")).startswith("actions/checkout") for step in _steps(job))


def test_release_triggers_on_tags_and_dispatch(release_workflow: dict[str, typ.Any]) -> None:
    """The release runs on a version tag push and on a dispatch with a tag."""
    triggers = release_workflow["on"]
    assert triggers["push"]["tags"] == ["v*.*.*"]
    dispatch_input = triggers["workflow_dispatch"]["inputs"]["tag"]
    assert dispatch_input["required"] is True


def test_release_workflow_defaults_to_read_permissions(
    release_workflow: dict[str, typ.Any],
) -> None:
    """Write scope is granted per job, not to the whole workflow."""
    assert release_workflow["permissions"] == {"contents": "read"}


@pytest.mark.parametrize("job_name", RELEASE_WRITING_JOBS)
def test_release_writing_jobs_request_write_scope(
    release_workflow: dict[str, typ.Any], job_name: str
) -> None:
    """Every job touching the release, including the auditor, may write."""
    job = release_workflow["jobs"][job_name]
    assert job["permissions"]["contents"] == "write"


def test_every_gh_step_has_repository_context(release_workflow: dict[str, typ.Any]) -> None:
    """`gh` resolves the repository from a checkout or from `GH_REPO`.

    Without either, `gh` falls back to reading a git remote and fails with
    `not a git repository`, skipping every downstream job.
    """
    for job_name, job in release_workflow["jobs"].items():
        checked_out = _has_checkout(job)
        for step in _steps(job):
            if not _invokes_gh(step):
                continue
            has_gh_repo = "GH_REPO" in (step.get("env") or {})
            assert checked_out or has_gh_repo, (
                f"{job_name}: step {step.get('name')!r} calls gh without a "
                "checkout or GH_REPO"
            )


def test_create_release_verifies_the_tag_through_the_api(
    release_workflow: dict[str, typ.Any],
) -> None:
    """The draft job checks the tag with the API because it has no clone."""
    job = release_workflow["jobs"]["create-release"]
    assert not _has_checkout(job)
    script = "\n".join(step.get("run", "") for step in _steps(job))
    assert "gh api" in script
    assert "git/ref/tags" in script
    assert "--verify-tag" not in script
    assert "--draft" in script


def test_build_matrix_covers_every_published_target(
    release_workflow: dict[str, typ.Any],
) -> None:
    """The matrix builds all five targets, each on a native runner."""
    entries = release_workflow["jobs"]["build-assets"]["strategy"]["matrix"]["include"]
    assert {entry["target"]: entry["runner"] for entry in entries} == EXPECTED_TARGET_RUNNERS


def test_build_assets_checks_out_the_release_tag(release_workflow: dict[str, typ.Any]) -> None:
    """The binary and the packaging script both come from the tagged tree."""
    job = release_workflow["jobs"]["build-assets"]
    checkout = next(
        step for step in _steps(job) if str(step.get("uses", "")).startswith("actions/checkout")
    )
    assert checkout["with"]["ref"] == "${{ needs.prepare.outputs.tag }}"


def test_upload_publishes_the_archive_and_its_sidecar(
    release_workflow: dict[str, typ.Any],
) -> None:
    """Both assets reach the release; a sidecar-less upload is the bug."""
    job = release_workflow["jobs"]["build-assets"]
    upload_index = _step_index(job, lambda step: "gh release upload" in step.get("run", ""))
    assert upload_index >= 0
    upload = _steps(job)[upload_index]
    assert "dist/*.tgz" in upload["run"]
    assert "dist/*.tgz.sha256" in upload["run"]


def test_the_sidecar_exists_before_the_upload(release_workflow: dict[str, typ.Any]) -> None:
    """Packaging and verification precede the upload step."""
    job = release_workflow["jobs"]["build-assets"]
    package_index = _step_index(job, lambda step: "release_archive.py" in step.get("run", ""))
    verify_index = _step_index(
        job, lambda step: "verify_release_archives.py" in step.get("run", "")
    )
    upload_index = _step_index(job, lambda step: "gh release upload" in step.get("run", ""))
    assert 0 <= package_index < verify_index < upload_index


def test_the_draft_is_audited_before_it_is_published(
    release_workflow: dict[str, typ.Any],
) -> None:
    """Publication is gated on the audit, and URL verification follows it."""
    jobs = release_workflow["jobs"]
    assert "build-assets" in jobs["audit-draft-assets"]["needs"]
    assert "audit-draft-assets" in jobs["publish-release"]["needs"]
    assert "publish-release" in jobs["verify-published-assets"]["needs"]
    audit_script = "\n".join(step.get("run", "") for step in _steps(jobs["audit-draft-assets"]))
    assert "verify_release_archives.py" in audit_script


def test_published_assets_are_resolved_without_compiling(
    release_workflow: dict[str, typ.Any],
) -> None:
    """The final job resolves the real URLs and never falls back to a build."""
    job = release_workflow["jobs"]["verify-published-assets"]
    script = "\n".join(step.get("run", "") for step in _steps(job))
    assert "cargo binstall" in script
    assert "--disable-strategies compile" in script
    for target in EXPECTED_TARGET_RUNNERS:
        assert target in script


def test_packaging_dry_run_covers_three_os_families(ci_workflow: dict[str, typ.Any]) -> None:
    """Pull requests package on Linux, macOS, and Windows."""
    job = ci_workflow["jobs"]["binstall-packaging"]
    entries = job["strategy"]["matrix"]["include"]
    assert {entry["os"] for entry in entries} == EXPECTED_PACKAGING_RUNNERS
    for entry in entries:
        assert entry["target"] in EXPECTED_TARGET_RUNNERS
    script = "\n".join(step.get("run", "") for step in _steps(job))
    assert "release_archive.py" in script
    assert "verify_release_archives.py" in script


@pytest.mark.parametrize("target", sorted(EXPECTED_TARGET_RUNNERS))
def test_binstall_templates_render_the_published_layout(
    binstall_metadata: dict[str, str], target: str
) -> None:
    """`pkg-url` and `bin-dir` render the names the packager writes."""
    version = tomllib.loads(MANIFEST_PATH.read_text(encoding="utf-8"))["package"]["version"]
    fields = {
        "repo": "https://github.com/leynos/ortho-config",
        "name": PACKAGE_NAME,
        "bin": PACKAGE_NAME,
        "target": target,
        "version": version,
        "binary-ext": naming.binary_extension(target),
        "archive-suffix": naming.ARCHIVE_SUFFIX,
    }
    url = naming.render_binstall_template(binstall_metadata["pkg-url"], fields)
    assert url.startswith("https://github.com/leynos/ortho-config/releases/download/v")
    assert url.endswith(naming.archive_file_name(PACKAGE_NAME, target, version))
    member = naming.render_binstall_template(binstall_metadata["bin-dir"], fields)
    assert member == naming.archive_member_path(PACKAGE_NAME, target, version, PACKAGE_NAME)


def test_binstall_format_is_the_published_format(binstall_metadata: dict[str, str]) -> None:
    """The declared `pkg-fmt` is the format the packager produces."""
    assert binstall_metadata["pkg-fmt"] == "tgz"
