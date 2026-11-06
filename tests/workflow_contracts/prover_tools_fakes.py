"""Fake tool boundaries shared by the prover Make contract tests.

The tests drive the real Make recipes, so the only thing they replace is the
child-process boundary owned by the shared package: `uv`, `rustup`, and the
`cargo` executables the installer invokes. Each stand-in records its argv as
one JSON line, which lets a test assert the exact command the recipe built
without claiming anything about the verifiers themselves.
"""

from __future__ import annotations

import json
import os
import shlex
import subprocess
import textwrap
from pathlib import Path
from typing import NamedTuple

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]


def write_executable(path: Path, content: str) -> Path:
    """Write an executable fixture script and return its path."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(textwrap.dedent(content).lstrip(), encoding="utf-8")
    path.chmod(0o755)
    return path


def fake_uv(tmp_path: Path) -> tuple[Path, Path]:
    """Create a `uv` stand-in that captures argv and exercises package parsing."""
    capture = tmp_path / "prover-tools-calls.jsonl"
    uv = write_executable(
        tmp_path / "bin" / "uv",
        """
        #!/usr/bin/env python3
        import json
        import os
        import shlex
        import subprocess
        import sys
        from pathlib import Path

        record = {
            "args": sys.argv[1:],
            "cargo_home": os.environ.get("CARGO_HOME"),
            "kani": os.environ.get("KANI"),
            "path": os.environ.get("PATH"),
            "verus_z3_path": os.environ.get("VERUS_Z3_PATH"),
        }
        with Path(os.environ["PROVER_TOOLS_CAPTURE"]).open("a", encoding="utf-8") as output:
            output.write(json.dumps(record) + "\\n")

        if "kani" in sys.argv and "install" in sys.argv:
            if "--compile" in sys.argv:
                subprocess.run(
                    ["cargo", "install", "--locked", "kani-verifier", "--version", "0.67.0"],
                    check=True,
                )
            subprocess.run(["cargo", "kani", "setup"], check=True)
        if "kani" in sys.argv and "check-version" in sys.argv:
            command_value = sys.argv[sys.argv.index("--kani-command") + 1]
            command = shlex.split(command_value)
            subprocess.run([*command, "--version"], check=True)
        """,
    )
    return uv, capture


def fake_rustup(tmp_path: Path) -> tuple[Path, Path]:
    """Create a `rustup` stand-in that records the toolchain it is asked for."""
    log = tmp_path / "rustup-invocations.jsonl"
    rustup = write_executable(
        tmp_path / "bin" / "rustup",
        f"""
        #!/usr/bin/env python3
        import json
        import sys
        from pathlib import Path
        with Path({str(log)!r}).open("a", encoding="utf-8") as output:
            output.write(json.dumps({{"args": sys.argv[1:]}}) + "\\n")
        """,
    )
    return rustup, log


def read_records(capture: Path) -> list[dict[str, object]]:
    """Return captured fake-package invocations in call order."""
    captured = capture.read_text(encoding="utf-8").splitlines()
    return [json.loads(line) for line in captured]


def recording_script(log: Path, *, banner: str = "") -> str:
    """Return a script that appends one JSON line per invocation to `log`.

    A banner is printed after recording so a caller that reads the tool's
    version, such as `kani-check`, sees a recognisable value.
    """
    lines = [
        "#!/usr/bin/env python3",
        "import json",
        "import sys",
        "from pathlib import Path",
        f"with Path({str(log)!r}).open('a', encoding='utf-8') as output:",
        "    record = {'path': sys.argv[0], 'args': sys.argv[1:]}",
        "    output.write(json.dumps(record) + '\\n')",
    ]
    if banner:
        lines.append(f"print({banner!r})")
    return "\n".join(lines) + "\n"


class KaniLayout(NamedTuple):
    """Paths and invocation logs the `CARGO_HOME` contract test inspects."""

    cargo_home: Path
    cargo_bin: Path
    conflict_bin: Path
    capture: Path
    cargo_log: Path
    kani_log: Path


def kani_environment(
    tmp_path: Path, *, explicit_cargo_home: bool
) -> tuple[dict[str, str], KaniLayout]:
    """Build the fake-tool environment for the `CARGO_HOME` contract test.

    A conflicting `cargo` earlier on `PATH` reproduces the donor installer bug,
    and the chosen home contains spaces and an apostrophe so both shell quoting
    and the package's `shlex.split` boundary are exercised.
    """
    home = tmp_path / "home dir o'brien"
    cargo_home = (
        tmp_path / "cargo home o'brien" if explicit_cargo_home else home / ".cargo"
    )
    layout = KaniLayout(
        cargo_home=cargo_home,
        cargo_bin=cargo_home / "bin",
        conflict_bin=tmp_path / "tools prefix" / "bin",
        capture=tmp_path / "prover-tools-calls.jsonl",
        cargo_log=tmp_path / "cargo-invocations.jsonl",
        kani_log=tmp_path / "kani-invocations.jsonl",
    )
    uv, capture = fake_uv(tmp_path)
    env = os.environ.copy()
    env.update(
        {
            "CARGO_HOME": str(cargo_home),
            "HOME": str(home),
            "PATH": os.pathsep.join((str(layout.conflict_bin), "/usr/bin", "/bin")),
            "PROVER_TOOLS_CAPTURE": str(capture),
            "UV": str(uv),
        }
    )
    if not explicit_cargo_home:
        env.pop("CARGO_HOME")
    env.pop("KANI", None)

    write_executable(layout.cargo_bin / "cargo", recording_script(layout.cargo_log))
    write_executable(
        layout.conflict_bin / "cargo",
        """
        #!/usr/bin/env python3
        import sys
        raise SystemExit("conflicting cargo selected: " + sys.argv[0])
        """,
    )
    write_executable(
        layout.cargo_bin / "cargo-kani",
        recording_script(layout.kani_log, banner="Kani 0.67.0"),
    )
    return env, layout


def assert_kani_install_calls(layout: KaniLayout) -> None:
    """Assert the installer used the selected Cargo home for both subprocesses."""
    cargo_invocations = [
        json.loads(line)
        for line in layout.cargo_log.read_text(encoding="utf-8").splitlines()
    ]
    assert cargo_invocations == [
        {
            "path": str(layout.cargo_bin / "cargo"),
            "args": ["install", "--locked", "kani-verifier", "--version", "0.67.0"],
        },
        {"path": str(layout.cargo_bin / "cargo"), "args": ["kani", "setup"]},
    ]


def assert_kani_check_call(
    layout: KaniLayout, records: list[dict[str, object]]
) -> None:
    """Assert `kani-check` selected the binary inside the chosen Cargo home."""
    kani_invocation = json.loads(layout.kani_log.read_text(encoding="utf-8").strip())
    assert kani_invocation == {
        "path": str(layout.cargo_bin / "cargo-kani"),
        "args": ["--version"],
    }
    check_arguments = records[1]["args"]
    command_value = check_arguments[check_arguments.index("--kani-command") + 1]
    assert shlex.split(command_value) == [str(layout.cargo_bin / "cargo-kani")]
    assert str(REPOSITORY_ROOT) in check_arguments
    assert records[0]["path"].split(os.pathsep) == [
        str(layout.cargo_bin),
        str(layout.conflict_bin),
        "/usr/bin",
        "/bin",
    ]


def run_make(
    target: str,
    *,
    env: dict[str, str],
    variables: tuple[str, ...] = (),
) -> subprocess.CompletedProcess[str]:
    """Run one repository Make target with caller-supplied environment and flags."""
    return subprocess.run(
        ["make", "--no-print-directory", *variables, target],
        cwd=REPOSITORY_ROOT,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )
