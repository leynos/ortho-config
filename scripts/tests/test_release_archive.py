"""Tests for the release archive packager, auditor, and binstall contract.

The archive layout is a contract with cargo-binstall: `pkg-url` names the
asset and `bin-dir` names the member inside it. These tests render the real
`[package.metadata.binstall]` templates from `cargo-orthohelp/Cargo.toml` and
compare them against what the packager actually produces, so a change to
either side fails here rather than in a consumer's install.
"""

from __future__ import annotations

import gzip
import hashlib
import sys
import tarfile
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_DIRECTORY = REPOSITORY_ROOT / "scripts"
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import release_archive as packager  # noqa: E402
import release_archive_naming as naming  # noqa: E402
import verify_release_archives as auditor  # noqa: E402

MANIFEST_PATH = REPOSITORY_ROOT / "cargo-orthohelp" / "Cargo.toml"
VERSION = "1.2.3"

#: Every target the release workflow publishes, paired with the binary suffix
#: Cargo produces for it.
TARGET_CASES = (
    ("x86_64-unknown-linux-gnu", ""),
    ("aarch64-unknown-linux-gnu", ""),
    ("x86_64-apple-darwin", ""),
    ("aarch64-apple-darwin", ""),
    ("x86_64-pc-windows-msvc", ".exe"),
)


@pytest.fixture(autouse=True)
def default_cargo_target_dir(monkeypatch: pytest.MonkeyPatch) -> None:
    """Package into the repository's own `target` directory during tests.

    Agents and shared build caches export `CARGO_TARGET_DIR`, which would
    otherwise send the packager looking outside the temporary repository.
    """
    monkeypatch.delenv("CARGO_TARGET_DIR", raising=False)


def _write_release_binary(repo: Path, target: str, payload: bytes = b"binary") -> Path:
    """Create a stand-in release binary where Cargo would have written one."""
    binary_path = packager.release_binary_path(repo, target)
    binary_path.parent.mkdir(parents=True, exist_ok=True)
    binary_path.write_bytes(payload)
    return binary_path


def _package(repo: Path, target: str, *, version: str = VERSION) -> Path:
    """Stage the archive and sidecar for *target* under `repo/dist`."""
    _write_release_binary(repo, target)
    archive_path = packager.stage_archive(
        packager.ReleaseArchiveSpec(
            repo=repo, target=target, version=version, dist_dir=repo / "dist"
        )
    )
    packager.write_checksum_sidecar(archive_path)
    return archive_path


@pytest.fixture(name="binstall_metadata")
def binstall_metadata_fixture() -> dict[str, str]:
    """Return the crate's real `[package.metadata.binstall]` table."""
    return naming.binstall_metadata(MANIFEST_PATH)


@pytest.mark.parametrize(("target", "extension"), TARGET_CASES)
def test_binary_extension_matches_target_family(target: str, extension: str) -> None:
    """Only Windows targets gain an `.exe` suffix."""
    assert naming.binary_extension(target) == extension


@pytest.mark.parametrize(("target", "_extension"), TARGET_CASES)
def test_pkg_url_renders_the_archive_file_name(
    binstall_metadata: dict[str, str], target: str, _extension: str
) -> None:
    """The `pkg-url` template's last path segment is the staged archive name."""
    rendered = naming.render_binstall_template(
        binstall_metadata["pkg-url"],
        {
            "repo": "https://github.com/leynos/ortho-config",
            "name": auditor.PACKAGE_NAME,
            "target": target,
            "version": VERSION,
            "archive-suffix": naming.ARCHIVE_SUFFIX,
        },
    )
    expected = naming.archive_file_name(auditor.PACKAGE_NAME, target, VERSION)
    assert rendered.rsplit("/", maxsplit=1)[-1] == expected


@pytest.mark.parametrize(("target", "_extension"), TARGET_CASES)
def test_staged_archive_matches_the_bin_dir_template(
    binstall_metadata: dict[str, str], tmp_path: Path, target: str, _extension: str
) -> None:
    """The archive holds exactly the member `bin-dir` renders for the target."""
    archive_path = _package(tmp_path, target)
    with tarfile.open(archive_path, mode="r:gz") as tar:
        members = [entry.name for entry in tar.getmembers()]
    expected = auditor.expected_member(binstall_metadata, target, VERSION)
    assert members == [expected]


def test_pkg_fmt_matches_the_archive_suffix(binstall_metadata: dict[str, str]) -> None:
    """`pkg-fmt` names the format the packager writes."""
    assert binstall_metadata["pkg-fmt"] == "tgz"
    assert naming.ARCHIVE_SUFFIX == ".tgz"


def test_packaged_binary_is_executable(tmp_path: Path) -> None:
    """The archived binary keeps a fixed executable mode."""
    archive_path = _package(tmp_path, "x86_64-unknown-linux-gnu")
    with tarfile.open(archive_path, mode="r:gz") as tar:
        entry = tar.getmembers()[0]
    assert entry.mode == 0o755


def test_archives_are_reproducible(tmp_path: Path) -> None:
    """Repackaging the same binary yields byte-identical archives."""
    first = _package(tmp_path / "one", "x86_64-unknown-linux-gnu")
    second = _package(tmp_path / "two", "x86_64-unknown-linux-gnu")
    assert first.read_bytes() == second.read_bytes()


def test_archive_records_no_wall_clock_timestamp(tmp_path: Path) -> None:
    """Neither the gzip header nor the member metadata carries the build time."""
    archive_path = _package(tmp_path, "x86_64-unknown-linux-gnu")
    with gzip.GzipFile(archive_path, mode="rb") as gz:
        gz.read(1)
        assert gz.mtime == 0
    with tarfile.open(archive_path, mode="r:gz") as tar:
        entry = tar.getmembers()[0]
    assert entry.mtime == 0
    assert (entry.uid, entry.gid, entry.uname, entry.gname) == (0, 0, "", "")


def test_sidecar_uses_the_sha256sum_format(tmp_path: Path) -> None:
    """The sidecar records the digest and the bare archive file name."""
    archive_path = _package(tmp_path, "x86_64-unknown-linux-gnu")
    sidecar_path = archive_path.with_name(f"{archive_path.name}.sha256")
    digest, name = sidecar_path.read_text(encoding="utf-8").split()
    assert digest == hashlib.sha256(archive_path.read_bytes()).hexdigest()
    assert name == archive_path.name


def test_staging_rejects_a_missing_binary(tmp_path: Path) -> None:
    """Staging fails loudly rather than publishing an empty archive."""
    spec = packager.ReleaseArchiveSpec(
        repo=tmp_path,
        target="x86_64-unknown-linux-gnu",
        version=VERSION,
        dist_dir=tmp_path / "dist",
    )
    with pytest.raises(SystemExit, match="release binary not found"):
        packager.stage_archive(spec)


def test_version_mismatch_is_rejected() -> None:
    """A requested version that differs from the manifest aborts packaging."""
    with pytest.raises(SystemExit, match="does not match"):
        packager.resolve_version(MANIFEST_PATH, "0.0.1-not-the-manifest-version")


def test_manifest_version_resolves_workspace_inheritance(tmp_path: Path) -> None:
    """`version.workspace = true` resolves against the workspace manifest."""
    (tmp_path / "Cargo.toml").write_text(
        '[workspace.package]\nversion = "4.5.6"\n', encoding="utf-8"
    )
    member = tmp_path / "member"
    member.mkdir()
    manifest = member / "Cargo.toml"
    manifest.write_text(
        '[package]\nname = "member"\nversion.workspace = true\n', encoding="utf-8"
    )
    assert naming.manifest_version(manifest) == "4.5.6"


def test_render_rejects_an_unknown_placeholder() -> None:
    """An unsupplied placeholder raises rather than rendering a wrong URL."""
    with pytest.raises(KeyError):
        naming.render_binstall_template("{ mystery }", {"name": "x"})


def _audit(dist_dir: Path, target: str, metadata: dict[str, str]) -> list[str]:
    """Run the auditor over one target and return its failures."""
    return auditor.audit_target(dist_dir, metadata, target, VERSION)


def test_auditor_accepts_a_well_formed_archive(
    binstall_metadata: dict[str, str], tmp_path: Path
) -> None:
    """A freshly staged archive passes the audit."""
    _package(tmp_path, "x86_64-unknown-linux-gnu")
    assert _audit(tmp_path / "dist", "x86_64-unknown-linux-gnu", binstall_metadata) == []


def test_auditor_reports_a_missing_archive(
    binstall_metadata: dict[str, str], tmp_path: Path
) -> None:
    """A target with no asset is reported rather than silently skipped."""
    failures = _audit(tmp_path / "dist", "aarch64-apple-darwin", binstall_metadata)
    assert failures and "is missing" in failures[0]


def test_auditor_reports_a_missing_sidecar(
    binstall_metadata: dict[str, str], tmp_path: Path
) -> None:
    """Deleting the sidecar fails the audit."""
    archive_path = _package(tmp_path, "x86_64-unknown-linux-gnu")
    archive_path.with_name(f"{archive_path.name}.sha256").unlink()
    failures = _audit(tmp_path / "dist", "x86_64-unknown-linux-gnu", binstall_metadata)
    assert any("sha256 is missing" in failure for failure in failures)


def test_auditor_reports_a_checksum_mismatch(
    binstall_metadata: dict[str, str], tmp_path: Path
) -> None:
    """A tampered archive no longer matches its recorded digest."""
    archive_path = _package(tmp_path, "x86_64-unknown-linux-gnu")
    archive_path.write_bytes(archive_path.read_bytes() + b"tamper")
    failures = _audit(tmp_path / "dist", "x86_64-unknown-linux-gnu", binstall_metadata)
    assert any("hashes to" in failure for failure in failures)


def test_auditor_reports_a_wrong_member_path(
    binstall_metadata: dict[str, str], tmp_path: Path
) -> None:
    """An archive whose member ignores `bin-dir` fails the audit."""
    dist_dir = tmp_path / "dist"
    dist_dir.mkdir(parents=True)
    target = "x86_64-unknown-linux-gnu"
    archive_path = dist_dir / naming.archive_file_name(auditor.PACKAGE_NAME, target, VERSION)
    with tarfile.open(archive_path, mode="w:gz") as tar:
        info = tarfile.TarInfo("cargo-orthohelp")
        info.size = 0
        tar.addfile(info)
    packager.write_checksum_sidecar(archive_path)
    failures = _audit(dist_dir, target, binstall_metadata)
    assert any("expected exactly" in failure for failure in failures)


def test_cargo_target_dir_override_is_honoured(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """An absolute `CARGO_TARGET_DIR` redirects where the binary is sought."""
    monkeypatch.setenv("CARGO_TARGET_DIR", str(tmp_path / "elsewhere"))
    path = packager.release_binary_path(tmp_path, "x86_64-unknown-linux-gnu")
    assert path == tmp_path / "elsewhere" / "x86_64-unknown-linux-gnu" / "release" / "cargo-orthohelp"


def test_relative_cargo_target_dir_resolves_against_the_repository(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """A relative override is resolved against the repository, as Cargo does."""
    monkeypatch.setenv("CARGO_TARGET_DIR", "build-out")
    assert packager.cargo_target_root(tmp_path) == tmp_path / "build-out"


def test_release_targets_cover_the_five_published_triples() -> None:
    """The auditor's target list matches the documented publication set."""
    assert set(auditor.RELEASE_TARGETS) == {target for target, _ in TARGET_CASES}
