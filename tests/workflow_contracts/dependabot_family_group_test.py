"""Contract for the Dependabot group that keeps ortho_config and fluent-bundle together.

``ortho_config`` and ``fluent-bundle`` share ``FluentValue`` types, so the two
must resolve to one ``fluent-bundle``. The Concordat dependency-family-contracts
rule (family ``ortho-config-fluent-bundle``) reads the Cargo entry of
``.github/dependabot.yml`` and wants one group that takes both members and
precedes the catch-all, because Dependabot uses the first group that matches. In
this repository ``ortho_config`` is a workspace crate, so Dependabot never offers
it, but the group still keeps ``fluent-bundle`` out of the catch-all.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import typing as typ

import yaml

CONFIG: typ.Final[pathlib.Path] = (
    pathlib.Path(__file__).resolve().parents[2] / ".github" / "dependabot.yml"
)
GROUP: typ.Final[str] = "ortho-config-fluent-bundle"
MEMBERS: typ.Final[list[str]] = ["ortho_config", "fluent-bundle"]
CATCH_ALL: typ.Final[str] = "minor-and-patch"


def _cargo_groups() -> dict[str, dict[str, typ.Any]]:
    """Return the root Cargo entry's groups, in file order."""
    document = yaml.safe_load(CONFIG.read_text(encoding="utf-8"))
    entries = [
        entry
        for entry in document["updates"]
        if entry["package-ecosystem"] == "cargo" and entry["directory"] == "/"
    ]
    assert len(entries) == 1, f"expected one root cargo entry; got {len(entries)}"
    return entries[0]["groups"]


def test_the_family_group_takes_both_members() -> None:
    """One group names ortho_config and fluent-bundle together."""
    groups = _cargo_groups()
    assert GROUP in groups, f"the cargo entry needs {GROUP!r}; got {list(groups)}"
    assert groups[GROUP]["patterns"] == MEMBERS, (
        f"{GROUP} must take exactly {MEMBERS}; got {groups[GROUP]['patterns']}"
    )


def test_the_family_group_precedes_the_catch_all() -> None:
    """Dependabot uses the first matching group, so the family comes first."""
    names = list(_cargo_groups())
    assert CATCH_ALL in names, f"the cargo entry needs the {CATCH_ALL!r} catch-all"
    assert names.index(GROUP) < names.index(CATCH_ALL), (
        f"{GROUP} must precede {CATCH_ALL}; got order {names}"
    )
